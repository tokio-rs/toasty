# Atomic Statement Execution

## Problem Statement

When a user submits a single `db.exec(stmt)` call, Toasty may issue **multiple
database queries** to fulfill the operation. This happens in two categories:

**Mutations with associations** — inserting, updating, or deleting records that
have associations requires multiple queries to maintain referential integrity:

- `User::create().with_profile(...)` → INSERT user + INSERT profile
- `user.update().set_profile(p)` → UPDATE user FK + UPDATE/INSERT child
- `user.delete()` → DELETE children (cascade) + DELETE user

**Queries with association preloads** — loading a model with its associations
requires one query per association level:

- `User::find(...).with_todos().all()` → SELECT users + SELECT todos WHERE
  user_id IN (...)

Currently, none of these multi-query plans execute atomically. Another
transaction can observe partial write state between queries, and a failure
midway leaves the database partially mutated. Similarly, preload queries can
observe inconsistent snapshots (e.g., a user is deleted between the first and
second SELECT), though the risk is minor in practice — see the Isolation Level
section below.

## Current State

The engine uses a single `PoolConnection` for the entire duration of an
`ExecPlan` (`engine/exec.rs`). This is the necessary precondition for
transactional execution: all queries go to the same connection.

Transaction operations (`Operation::Transaction::Start/Commit/Rollback`) already
exist in the driver interface and are implemented in all SQL drivers.
Multi-query `ExecPlan`s are now wrapped in `BEGIN ... COMMIT`. `ReadModifyWrite`
uses savepoints for its nested atomic boundary.

**This design is fully implemented** (see Implementation Plan below). The only
remaining work is integration tests.

## Scope

This document covers only **engine-internal atomicity**: making a single
`db.exec()` call atomic across all its internal queries.

**Out of scope**: user-level explicit transactions (`db.begin_transaction()` to
group multiple `db.exec()` calls). That is a separate feature that builds on
top of this.

**Out of scope for initial implementation**: DynamoDB. DynamoDB has no
traditional transaction semantics (it offers `TransactWriteItems` and
`TransactGetItems` as separate batch APIs). DynamoDB atomicity requires its own
design pass.

## Proposed Solution: Automatic Transaction Wrapping (Option A)

Modify `engine/exec.rs` to wrap every `ExecPlan` that issues multiple database
operations in a `BEGIN ... COMMIT` block, with a `ROLLBACK` on any failure.

```
exec_plan():
  if plan.needs_transaction:
    connection.exec(Transaction::Start)
    for each action:
      execute action
      on error:
        connection.exec(Transaction::Rollback)
        return error
    connection.exec(Transaction::Commit)
  else:
    for each action:
      execute action
```

This includes **read-only plans** with association preloads. If a SELECT plan
issues more than one DB query (i.e., it preloads associations), it too is
wrapped in a transaction. This ensures a consistent snapshot: all queries in the
plan see the same database state.

Single-action plans are **not** wrapped (no overhead for the common case).

### Why not make transaction boundaries explicit in the plan?

An alternative ("Option B") would have the execution planner emit explicit
`BeginTransaction`/`CommitTransaction` actions into the `ExecPlan`. This makes
the transaction boundary visible in the plan but requires changes across the
compilation pipeline. Option A is simpler: the logic lives in one place, and the
rule is easy to document: "any `ExecPlan` with more than one database operation
executes within a transaction."

## ReadModifyWrite and Nested Transactions

`ReadModifyWrite` currently issues `Transaction::Start` and `Transaction::Commit`
itself. With the outer transaction wrapping the plan, the RMW's inner `BEGIN`
would be a **nested transaction**, which most databases do not support via
`BEGIN`.

The correct tool for nested atomic boundaries within an existing transaction is
**savepoints**. Add three new variants to `Transaction`:

```rust
pub enum Transaction {
    Start,
    Commit,
    Rollback,
    Savepoint(usize),         // SAVEPOINT <id>
    ReleaseSavepoint(usize),  // RELEASE SAVEPOINT <id>  (commit)
    RollbackToSavepoint(usize), // ROLLBACK TO SAVEPOINT <id>
}
```

Savepoints are identified by a `usize`. The driver (via `toasty-sql` for SQL
backends) is responsible for converting the numeric identifier to a valid
savepoint name in the emitted SQL (e.g., `sp_1`, `sp_2`). This follows the
project convention of using numeric identifiers rather than user-facing strings
in the engine layer.

Update `ReadModifyWrite` to use savepoints instead of `Start`/`Commit`:

```
action_read_modify_write():
  id = self.generate_savepoint_id()   // monotonically increasing usize per plan
  connection.exec(Transaction::Savepoint(id))
  execute read query
  check condition — on failure:
    connection.exec(Transaction::RollbackToSavepoint(id))
    return error  ← outer loop will then rollback the whole plan
  execute write query
  connection.exec(Transaction::ReleaseSavepoint(id))
```

`generate_savepoint_id()` is a method on `Exec` that increments a counter
starting at 0 for each plan execution (`next_savepoint_id: usize`). This
ensures multiple RMW actions within the same plan use distinct savepoint IDs
(`sp_0`, `sp_1`, ...) and avoids collisions.

This also fixes the existing bug: the RMW currently has no rollback path on
condition failure. With savepoints, `RollbackToSavepoint` explicitly undoes any
work the RMW started before returning the error.

Note: `RELEASE SAVEPOINT` does not commit the outer transaction — it merges the
savepoint's changes into the enclosing transaction. The outer transaction still
commits or rolls back as a whole.

For SQLite, savepoints work as nested transactions even without an outer `BEGIN`
(SQLite promotes a `SAVEPOINT` to a transaction if not already in one). This
means the current behavior is preserved if a plan happens to have only an RMW
and no outer transaction wrapping.

## Capability Model

No new capability flag is needed. The existing `capability.sql` flag already
captures the distinction: all SQL drivers support transactions, and the only
non-SQL driver (DynamoDB) does not. The engine uses `capability.sql` to gate
whether to wrap a plan in a transaction.

| Driver     | `sql`   | Transactions |
|------------|---------|--------------|
| SQLite     | `true`  | implemented |
| PostgreSQL | `true`  | implemented |
| MySQL      | `true`  | implemented |
| DynamoDB   | `false` | not supported (deferred) |

For drivers where `capability.sql` is `false`, the engine falls back to the
current non-transactional behavior.

## ExecPlan Tracks Transaction Requirement

The execution planner (`engine/plan/execution.rs`) is already aware of the
driver's capabilities and knows which actions it is emitting. It adds a
`needs_transaction` field to `ExecPlan`:

```rust
pub(crate) struct ExecPlan {
    pub(crate) vars: VarStore,
    pub(crate) actions: Vec<Action>,
    pub(crate) returning: Option<VarId>,
    /// Whether the executor should wrap this plan in a transaction.
    pub(crate) needs_transaction: bool,
}
```

The execution planner sets `needs_transaction = true` when `capability.sql` is
true and more than one action in the plan issues a database operation.

Actions that count as database operations: `ExecStatement`, `GetByKey`,
`DeleteByKey`, `UpdateByKey`, `QueryPk`, `FindPkByIndex`, `ReadModifyWrite`.
In-memory actions do not count: `Filter`, `Project`, `NestedMerge`, `SetVar`,
`Eval`.

`exec_plan()` then simply reads the flag:

```
exec_plan():
  if plan.needs_transaction:
    connection.exec(Transaction::Start)
    for each action:
      execute action
      on error:
        connection.exec(Transaction::Rollback)
        return error
    connection.exec(Transaction::Commit)
  else:
    for each action:
      execute action
```

## Error Handling

On any action failure:

1. Issue `Transaction::Rollback`
2. If the rollback itself fails, log the rollback error and return the original
   error (the connection is likely broken at this point)
3. The connection is returned to the pool after the `PoolConnection` is dropped

## Isolation Level

Each database's default isolation level is used. No explicit `SET TRANSACTION
ISOLATION LEVEL` is issued.

| Driver     | Default isolation    | Behavior for read plans |
|------------|---------------------|-------------------------|
| SQLite     | Serializable (WAL: snapshot) | Consistent snapshot for the transaction lifetime |
| MySQL      | Repeatable Read     | Consistent snapshot from first read |
| PostgreSQL | Read Committed      | Each query sees latest committed state at the time it runs |

The PostgreSQL default means preload queries within a single plan are not
fully snapshot-consistent: a concurrent transaction committing between the
first and second SELECT is visible to the second SELECT. The anomaly window is
small (milliseconds between two fast queries) and this matches the behavior of
most ORMs. It is documented as a known limitation.

If tighter read consistency is needed on PostgreSQL it can be addressed as a
follow-up by explicitly setting `REPEATABLE READ` when starting the transaction
for read-only plans.

## Streaming Results

When a plan returns a streaming result, the transaction is held open for the
lifetime of the stream. This is intentional — the stream is still part of the
plan's execution and must see a consistent view. The connection is released back
to the pool once the stream is fully consumed or dropped.

## Open Questions

1. **DynamoDB**: Needs its own design. `TransactWriteItems` supports up to 25
   atomic write operations. A future `Capability::max_atomic_write_ops:
   Option<usize>` field could express this limit.

## Implementation Plan

- [x] 1. Add `Savepoint(usize)`, `ReleaseSavepoint(usize)`, `RollbackToSavepoint(usize)` variants to `Transaction`
  - File: `crates/toasty-core/src/driver/operation/transaction.rs`
- [x] 2. Centralize transaction SQL in `toasty-sql`; update all drivers to delegate to it
  - Added `Serializer::serialize_transaction(&Transaction) -> String`
  - MySQL: `START TRANSACTION`; all others: `BEGIN`; savepoints: `sp_{id}`
  - Files: `crates/toasty-sql/src/serializer.rs`, `crates/toasty-driver-sqlite/src/lib.rs`,
    `crates/toasty-driver-postgresql/src/lib.rs`, `crates/toasty-driver-mysql/src/lib.rs`
- [x] 3. Add `is_db_op()` to `Action` using exhaustive match (compile error on new variants)
  - File: `crates/toasty/src/engine/exec/action.rs`
- [x] 4. Add `needs_transaction: bool` to `ExecPlan`; compute it in `ExecPlanner::plan_execution`
  - `ExecPlanner` gains `use_transactions: bool` (set from `capability.sql`)
  - Files: `crates/toasty/src/engine/exec/plan.rs`, `crates/toasty/src/engine/plan.rs`,
    `crates/toasty/src/engine/plan/execution.rs`
- [x] 5. Wrap `exec_plan()` in `BEGIN ... COMMIT` with rollback on error
  - Added `next_savepoint_id: usize` and `generate_savepoint_id()` to `Exec`
  - File: `crates/toasty/src/engine/exec.rs`
- [x] 6. Update `ReadModifyWrite` to use savepoints; fix rollback-on-failure bug
  - File: `crates/toasty/src/engine/exec/rmw.rs`
- [x] 7. Integration tests: verify partial-failure rollback across all SQL drivers
  - File: `crates/toasty-driver-integration-suite/src/tests/tx_atomic_stmt.rs`
