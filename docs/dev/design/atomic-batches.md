# Atomic Batches for DynamoDB

## Summary

Today, `toasty::batch()` on DynamoDB executes writes independently — a mid-batch failure leaves partial state. This design makes `toasty::batch()` and cascading writes atomic on every backend Toasty ships drivers for. A new driver capability, `Capability::transaction_delivery`, names how a driver receives the writes of an atomic group:

* `Unsupported` - drivers that do not support atomic multi-write operations
* `Streamed` - the SQL backends, which take the writes one at a time under engine-controlled `BEGIN/COMMIT`
* `WriteSet` - DynamoDB, which takes them as a single `Operation::TransactWrite` it must commit or cancel together

The engine emits `TransactWrite` for a `WriteSet` driver whenever a plan performs more than one write, instead of streaming the steps. SQL backends keep their existing statement-stream behavior; DynamoDB translates `TransactWrite` to `transact_write_items()`. The user-visible promise in [Batch Operations](../../guide/src/batch-operations.md) — "all operations in a batch either succeed together or fail together" — now holds on DynamoDB.

## Motivation

A user calling

```rust
toasty::batch((
User::create().email("new@example.com"),
Post::filter_by_id(id).update().title("hello"),
))
.exec(&mut db)
.await
```

against DynamoDB gets two independent writes, not an atomic transaction. If the second write fails, the first has already committed. The user guide acknowledges this with "database permitting," but the gap is real.

Cascading delete is another example. Deleting a `User` with a `HasMany<Todo>` association produces a plan with one delete per child plus one for the parent. On SQL these run in a single transaction; on DynamoDB they run as N independent `DeleteItem` calls. A failure mid-cascade leaves dangling children.

The DynamoDB driver already calls `transact_write_items()` in six places — the multi-item and unique-index branches of insert (`op/insert.rs`), delete (`op/delete_by_key.rs`), and update (`op/update_by_key.rs`). The engine never emits the shape that reaches those branches for a `toasty::batch()`, so the AWS SDK for Rust (SDK) plumbing exists but is unreachable from user code.

## User-facing API

No new public API. `toasty::batch()`, `create_many()`, and cascading writes work the same way they always have, but the atomicity guarantee in [Batch Operations](../../guide/src/batch-operations.md) now applies uniformly.

The wording in the guide can drop its hedge:

**Before:**

> Batch operations are **atomic**, database permitting — all operations in a batch either succeed together or fail together.

**After:**

> Batch operations are **atomic** on all databases Toasty ships drivers for — all operations in a batch either succeed together or fail together. Drivers that do not support atomic writes return `Error::unsupported_feature` when a multi-write batch is attempted.

A new section explains the per-backend ceiling:

> DynamoDB transactions are limited to 100 actions and 4 MB per transaction. A batch that exceeds this limit returns `Error::batch_too_large` rather than splitting silently. SQL backends have no such limit at the Toasty layer.

## Behavior

A plan that performs more than one write is atomic. A plan with a single write — one that resolves to exactly one item, with no surrounding `db.transaction()` scope — bypasses the transaction wrapper and runs as a single SDK call, the same as today. A single filter-based write step that matches multiple rows resolves to multiple items and so takes the atomic path; "one write" means one item, not one plan step.

On SQL backends nothing observable changes. The engine still wraps multi-step plans in `BEGIN/COMMIT` and uses savepoints when nested inside a `db.transaction()`.

A driver reporting `Unsupported` returns `Error::unsupported_feature` for any plan requiring more than one write.

On DynamoDB the engine emits `Operation::TransactWrite { items }` for a multi-step plan. The driver maps it to `transact_write_items()`. If the SDK call fails with `TransactionCanceledException`, the engine surfaces:

* `Error::condition_failed` if any cancellation reason was a conditional-check failure caused by an explicit `condition`.
* A successful empty/zero result if every conditional-check failure was caused by a `filter` predicate — the same filter-failure mapping the single-write update path already applies (`filter_failed` in `op/update_by_key.rs`), which returns an empty result rather than an error.
* `Error::driver_operation_failed` for any other SDK error.

Validation splits across two stages by what each check needs to know. _Structural_ checks depend only on the shape of the plan and run at planning time: the write-depends-on-write rejection below, and the same-table duplicate-key check. _Count_ checks depend on resolved data — how many rows a filter matched — and run after the read prefix, just before the `TransactWrite` is sent. Both reject before any write reaches the SDK; they differ only in how early they can.

A batch whose item count exceeds the `WriteSet { max_items }` ceiling returns `Error::batch_too_large` before the `TransactWrite` is sent. The item count is not always known at planning time: a filter-based update or delete compiles to a single write step that expands to one item per matching row, and the match count is only known after the preceding read resolves. The cap is therefore checked once the read prefix has run and the item count is final, before any write reaches the SDK — never partway through a committing transaction.

Cascading deletes inherit the same guarantee. A `User::delete()` with a `HasMany<Todo>` association produces a single `TransactWrite` carrying one `Delete` per affected `Todo` plus one for the `User`. The parent and children commit together or not at all.

## Edge cases

**Batch size limits.** DynamoDB caps a transaction at 100 actions and 4 MB. Toasty validates only the resolved item count — after the read prefix expands filter-based writes into per-row items — against the `max_items` field of `TransactionDelivery::WriteSet`. The 4 MB byte ceiling is left to the driver: computing serialized item sizes in the engine would duplicate the backend's own encoding rules, so an oversized batch surfaces as a driver error from the SDK. Splitting the batch across multiple transactions would lose atomicity, so Toasty refuses rather than splits.

**`returning` on multi-row updates.** `transact_write_items()` does not return `UPDATED_NEW`. A multi-row update with a `returning` clause that depends on the post-update value (e.g. arithmetic assignments) is unsupported on DynamoDB and returns `Error::unsupported_feature`. The single-row update path keeps `UPDATED_NEW` and behaves as today.

**Write-on-write data dependencies.** `TransactWrite` commits all items in one shot with no round-trip between them, so every write item must be buildable from data available _before_ the transaction begins. Toasty rejects, with `Error::unsupported_feature` at planning time, any batch where a write's input traces back to another write:

* `write → write` — one update's `returning` value feeds a second update's key or assignment.
* `write → read → write` — an intervening read depends on the first write's effect; it cannot run inside the transaction and cannot observe the uncommitted write.

A `read → write` batch is fine: the read runs in the prefix and resolves the keys the write operates on. Only a _write_ in a write item's data-ancestry is disqualifying; reads and transforms on the path are walked through, not stopped at.

The rejection is specific to `WriteSet` drivers. On SQL the engine streams writes inside `BEGIN/COMMIT`, where each statement's result and any read between them is visible to later steps.

**Same-table filter and static-key write conflicts.** A filter-based update or delete carries an implicit read that resolves which rows it touches. On a `WriteSet` driver that read runs in the prefix, against state from _before_ the transaction, so it cannot see any sibling write in the same batch. On SQL the equivalent read runs between the streamed statements and does see them. The results diverge silently:

```rust
batch((
Todo::find_by_id(7).update().status("active"),           // static-key write
Todo::filter(status = "active").update().archived(true), // filter write
))
```

On SQL the filter write observes todo 7's new status and archives it; on DynamoDB its prefix read runs first and misses todo 7. To keep results backend-independent, a `WriteSet` driver rejects, with `Error::unsupported_feature` at planning time, any batch in which a filter-based write shares its table with another write — whether that other write is static-key or itself filter-based. This is conservative by design: it refuses batches that might succeed rather than allow a silent cross-backend divergence. Two refinements are possible but deferred — allowing same-table filter writes when their assigned and filtered columns do not overlap, and exploiting write order (a filter write that precedes every same-table static-key write is safe). Both are deferred to v2.

This check and the write-depends-on-write rejection above share one plan-time structural pass; each reads `table`/`filter` data already present on the write nodes.

**Duplicate primary key within one `TransactWrite`.** DynamoDB rejects two writes to the same `(table, primary key)` in a single transaction. With the filter rule above in force, the only way two writes can reach the same table is when both name static keys — and those keys are literals known at planning time, so the collision is caught in the same structural pass with a clearer error than the SDK message. A collision between _filter-resolved_ keys (two filter writes whose matches overlap on a row neither names) cannot arise: the filter rule already rejects any batch with two same-table writes before keys are resolved. This is the one check whose stage is contingent — it can stay at planning time only because the filter rule is conservative. Relaxing that rule (the deferred disjoint-columns refinement) would let two filter writes share a table, at which point overlapping matches become possible and duplicate-key detection would have to move to the post-prefix stage, where the resolved key sets are known.

**Single-step plans.** A batch of one write — or a non-batched single write — bypasses the transaction wrapper. The user pays no transaction cost for trivially atomic operations.

**Nested batches inside `db.transaction()`.** SQL uses savepoints for nested scopes, same as today. DynamoDB has no nested-transaction support; calling `toasty::batch()` inside a `db.transaction()` on DynamoDB is rejected at runtime with `Error::unsupported_feature`, matching the existing rejection of `Operation::Transaction` on DynamoDB.

**Read-only batches.** Reads do not participate in `TransactWrite`. A batch containing only queries continues to use the existing `BatchGetItem`/`Query` paths on DynamoDB. Snapshot-consistent reads via `TransactReadItems` are out of scope here.

## Driver integration

Two new pieces in the driver interface.

Two **`Capability`** fields describe how the driver receives the writes of an atomic group:

```rust
pub struct Capability {

    /// When true, writes stream one at a time under engine-controlled
    /// BEGIN/COMMIT. When false, the engine hands the driver one
    /// Operation::TransactWrite to commit together.
    pub streamed_transactions: bool,

    /// Maximum writes a non-streamed driver commits in one atomic group, or
    /// None for no limit. Only meaningful when streamed_transactions is false.
    pub max_transaction_writes: Option<usize>,
    // …
}
```

SQL drivers report `streamed_transactions: true` and keep their `BEGIN/COMMIT` handling. DynamoDB reports `streamed_transactions: false, max_transaction_writes: Some(100)`. `streamed_transactions` is the gate that decides whether the engine streams the writes or hands the driver a single set — replacing `Capability::sql`, which conflated "is this SQL" with "how are atomic writes delivered" (MongoDB streams but is not SQL).

A boolean suffices because every database Toasty targets has atomic multi-write: SQL and MongoDB stream, DynamoDB takes a write set.&#x20;

**`Operation::TransactWrite`** is a new variant:

```rust
pub struct TransactWrite { pub items: Vec<TransactItem> }

pub enum TransactItem {
    Delete { table: TableId, key: stmt::Value, condition: Option<stmt::Expr> },
    Put    { table: TableId, row: stmt::ValueRecord, condition: Option<stmt::Expr> },
    Update { table: TableId, key: stmt::Value, assignments: stmt::Assignments, condition: Option<stmt::Expr> },
}
```

v1 emits only `Delete`, `Put`, and `Update`. A standalone condition-check item — asserting on a row the batch does not write — has no source: no Toasty API produces one (see [Out of scope](#out-of-scope)). It is left out of the enum until a feature generates it, rather than declared with no path to it.

A driver that reports `WriteSet` must implement `Connection::exec` for `Operation::TransactWrite`. The driver is responsible for translating each `TransactItem` to its native write-with-condition form. The DynamoDB driver maps `Delete`, `Put`, and `Update` to their `TransactWriteItem` builder counterparts; the existing `update_expression` and `filter`/`condition` composition logic is reused per item.

A driver that reports `Streamed` is unaffected. The existing `Operation::Transaction` start/commit/rollback flow stays in place, and `Operation::DeleteByKey` / `Operation::UpdateByKey` continue to handle their multi-key cases as today.

The DynamoDB driver simplifies on this side. A generic `exec_transact_write` and per-item bodies replace the multi-key transact branch in `op/delete_by_key.rs` and `op/update_by_key.rs`, and the multi-item transact branch in `op/insert.rs`.&#x20;

The insert migration is a driver-internal refactor, not an engine-emit change: a `create_many()` still compiles to a single `Operation::Insert` carrying many rows, and the engine does not stream it as multi-step writes. The driver, on receiving that op, builds one `Put` per row and commits them through the shared `exec_transact_write` helper.

This change also closes a pre-existing correctness gap in the DynamoDB driver. Today the no-unique-index arm splits three ways: a single row uses `put_item`, a multi-row insert with a version column uses `transact_write_items`, and a plain multi-row insert falls to `batch_write_item` — which is **not** atomic and can partially succeed. Since `create_many()` is a Toasty batch, it must be atomic, so the plain multi-row case moves onto the transaction path with the rest. The version-column split disappears: the transaction builder already adds the version condition only when one exists, so the conditioned and unconditioned multi-row inserts are the same call. The arm reduces to two cases — one row uses `put_item`, more than one uses `exec_transact_write` — and the multi-row path inherits the 100-action cap and `Error::batch_too_large` along with atomicity.

### Logical and physical expansion

The engine owns logical fan-out; the driver owns physical expansion. Keeping those two responsibilities separate is what lets the engine build `TransactItem`s without reasoning about backend storage details.

**Logical fan-out** is one user operation touching many rows — a filter-based delete that resolves to N primary keys. The engine resolves fan-out in the read prefix, before any write item is built. **Physical expansion** is one logical write becoming several physical table writes. On DynamoDB, a write to a unique-indexed table is the main-table write plus a write to each unique index's shadow table, which is how Toasty enforces uniqueness on a store with no native unique constraint.

`TransactItem` sits at the boundary between the two: it is a logical, single-row write. The driver expands each item into its native writes; the engine never sees shadow tables. `exec_transact_write` and the standalone insert/delete/update paths share one expansion routine, so unique-index maintenance is identical whether a write is committed alone or as part of a batch.

Two correctness notes follow from this split. First, a unique-column update or delete needs a pre-read to find the old shadow row to remove, and that read runs outside the transaction — a TOCTOU window. The shared expansion closes it the same way the single-operation path does: the main write carries `unique_col = :old_value` as a condition, so a concurrent change cancels the transaction at commit. Second, `max_transaction_writes` counts logical items, so physical expansion can push the action count above what the engine's pre-send check sees. An overflow surfaces as a driver-side SDK error, the same way the 4 MB byte limit does.

## Alternatives considered

**Keep transaction orchestration entirely in the driver.** Each driver would translate `Operation::Transaction(Start)` into its own multi-statement aggregator, buffering subsequent writes until `Commit`. This approach is rejected: it pushes the "how big can a transaction be" question into every driver and duplicates the cap logic. Centralizing on `Operation::TransactWrite` keeps the contract explicit and lets the planner refuse oversized batches before sending anything.

**Add a separate `Operation::TransactWrite` only for DynamoDB and leave SQL untouched.** This option is rejected for SQL. A SQL driver could translate `TransactWrite` to `BEGIN; <writes>; COMMIT`, but SQL already has `Operation::Transaction` and a streaming model that fits its protocol better. Keeping SQL on the streaming path avoids buffering plan output in memory just to flatten it back into statements.

**Make the engine split oversized batches automatically.** Splitting across multiple `transact_write_items()` calls would lose the atomicity the user asked for. Failing loudly with `Error::batch_too_large` is better than silently weakening a guarantee the API documents.

## Open questions

**Relax the same-table filter-write rejection to disjoint columns.** _Deferrable._ The filter rule rejects any batch where a filter-based write shares its table with another write. Allowing it when the assigned and filtered columns provably do not overlap would accept more batches, but moves duplicate-key detection from planning time to after the read prefix (the two checks are coupled — see the duplicate-key edge case). This analysis is deferred to v2.

**Exploit write order for same-table filter writes.** _Deferrable._ A filter write that precedes every same-table static-key write is safe even under the current rule, because its prefix read runs before any sibling write could affect it. Detecting this ordering would accept a further class of batches the conservative rule rejects. Deferred for the same reason.

## Out of scope

* **Atomic snapshot reads (`TransactReadItems`).** A separate gap with a separate fix; tracked alongside the read-batching audit, not here.
* **Standalone condition checks.** Asserting on a row the batch does not write — "delete this `Todo` only if its parent `User` is still active" — is unsupported on both backends, because no Toasty API produces a condition divorced from a write. Today a `condition` attaches only to a `delete()` or `update()` of the same row. The mechanism exists on each backend and is not DynamoDB-specific: on DynamoDB it is a `ConditionCheck` action in the transaction; on SQL it is an in-transaction `SELECT count(*) … FILTER (condition)` whose result the engine compares and, on mismatch, raises `Error::condition_failed` to trigger the same rollback path conditional updates already use. What is missing on both is the user-facing surface to express the assertion. Designing that surface — and the `TransactItem::ConditionCheck` variant it would feed — is a separate feature.
* **Cross-database atomic batches.** Mixing two driver instances in one batch is a roadmap item under [Transactions](../roadmap.md#transactions) and orthogonal to this design.
