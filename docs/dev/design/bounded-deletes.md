# Bounded deletes

## Summary

Preserve a query's `.limit(n)` when it becomes a delete, so retention jobs can remove at most `n`
target-model rows per execution. Add an opt-in `.exec_count()` returning the actual number deleted,
while keeping `.exec()` returning `()`. Supported SQL drivers execute one bounded delete statement;
unsupported drivers and query shapes return a capability error before writing. This is the proposed
contract for [#1328](https://github.com/tokio-rs/toasty/issues/1328), not an implemented API.

## Motivation

A retention job on a database that caps rows modified per transaction must drain a backlog in small
batches. An unrestricted filtered delete can exceed that budget. Today the application can express
the batch using raw SQL, then use the affected count to decide whether it made progress:

```sql
DELETE FROM runs
WHERE id IN (SELECT id FROM runs WHERE created_at < $1 LIMIT 2000);
```

The typed delete API returns `()`, and converting a query to a delete does not preserve its limit.
Applications therefore own the SQL dialect handling and count bookkeeping. The proposal lets the
application keep its model predicate and explicitly choose the batch size and transaction boundary.

## User-facing API

### Delete one bounded batch

Call `.limit(n)` on a model query before `.delete()`. The limit applies to the rows deleted, even
when you execute the delete without requesting a count:

```rust
Run::filter(Run::fields().created_at().lt(cutoff))
    .limit(2000)
    .delete()
    .exec(&mut db)
    .await?;
```

Call `.exec_count()` instead of `.exec()` when you need progress information:

```rust
let deleted: u64 = Run::filter(Run::fields().created_at().lt(cutoff))
    .limit(2000)
    .delete()
    .exec_count(&mut db)
    .await?;
```

The new method on `Delete<()>` consumes the statement and takes `&mut dyn Executor`, like `.exec()`.
Its result is `toasty::Result<u64>`. It also works for an unbounded single-target delete on a driver
that supports exact direct-delete counts. Existing `.exec()` calls keep their unit result; a query
with a limit now requests a bounded delete rather than silently losing that limit. Neither method
returns deleted models, and this proposal changes no update or query-count API.

### Repeat batches explicitly

Choose a fixed cutoff outside the loop, then execute each batch separately:

```rust
loop {
    let deleted = Run::filter(Run::fields().created_at().lt(cutoff))
        .limit(2000)
        .delete()
        .exec_count(&mut db)
        .await?;

    if deleted == 0 {
        break;
    }
}
```

This example stops when an execution makes no progress. A short batch, including zero, is not a
universal proof that no eligible rows remain: concurrent changes or triggers that suppress deletion
can leave matches behind. With a stable eligible set and no deletion-suppressing triggers, repeating
until zero drains the set. Production jobs should choose their own retry, time, and progress
budgets.

Use a separate committed execution for each batch when the purpose is to stay below a transaction
budget. Several bounded deletes inside one `db.transaction()` or atomic batch still accumulate
writes
in that transaction. Toasty does not infer a backend's transaction ceiling or reserve room for other
writes. A bound on target rows does not account for database-side triggers or foreign-key cascades.

### Choose a subset with ordering

Without `.order_by()`, the chosen subset is unspecified. To request the oldest eligible records
first,
order the query and supply a unique tie-breaker when stable selection matters:

```rust
let deleted = Run::filter(Run::fields().created_at().lt(cutoff))
    .order_by((Run::fields().created_at().asc(), Run::fields().id().asc()))
    .limit(2000)
    .delete()
    .exec_count(&mut db)
    .await?;
```

The order chooses the candidate subset. It does not promise the physical order in which deletes or
triggers execute, or snapshot consistency between calls. Native collation, null ordering, and
transaction isolation still apply. This proposal does not add an implicit ordering or key
tie-breaker.

### Before and after

Before this feature, use the raw SQL statement above and its affected-row count. After the feature
ships, use the equivalent model predicate with `.limit(...).delete().exec_count(...)`. Unbounded
`.delete().exec(...)` callers need no result-type migration.

## Behavior

For a supported bounded delete, one execution performs one SQL data-modification statement and
removes at most `n` rows directly from the target model's table. It selects at most `n` complete,
distinct primary keys matching the predicate, or uses an equivalent native single-table delete
limit.
The bound must be enforced by that statement, not by limiting returned rows or by a separate key
read.

The first version accepts model queries whose filter and ordering operate on the target table's
fields. It supports complete composite primary keys. It rejects relation traversal, includes,
projections, query locks, offsets (including an explicit zero offset), cursor limits, and explicit
write conditions. These shapes must not be silently discarded while converting to a delete.
Expanding the accepted shapes is follow-up work once their semantics can be preserved.

A bounded delete must also be rejected if Toasty would expand it into additional writes, including
relation cleanup, cascades, or a read-then-write mutation plan. The same restriction applies to
`.exec_count()` without a limit: its count must describe one direct target-table delete, not a sum
of
unrelated writes. Ordinary unbounded `.exec()` retains its existing behavior.

On success, `.exec_count()` returns the backend's number of target-table rows actually deleted,
excluding rows changed by triggers, database-side cascades, or auxiliary storage maintenance. Never
return the number of candidates selected, keys attempted, statements acknowledged, or rows matched
before deletion. An empty match returns `Ok(0)`; `.exec()` returns `Ok(())` for the same operation.

Unsupported capabilities, unsupported query shapes, and Toasty write expansion return
`Error::unsupported_feature`, recognizable with `.is_unsupported_feature()`, before any write is
sent. The error should identify the unsupported bounded/count behavior or query shape. A caller must
opt into any raw SQL or alternative strategy; there is no automatic unbounded fallback.

Backend execution failures retain the existing driver error classification. A failed or cancelled
operation does not return a successful count. A connection failure can leave commit outcome unknown;
this proposal introduces no transparent mutation replay. In an explicit transaction, the returned
count describes that statement's effect inside the transaction and does not guarantee later commit.

Single-statement execution introduces no read/write transaction requirement. Existing executor and
transaction capability rules still apply, including rejection of a surrounding transaction a driver
cannot support. Multiple limited deletes in one batch each have their own bound, not a shared
budget.

## Edge cases

- **Zero.** After capability and query-shape validation, `.limit(0)` succeeds without a data
  statement
  or mutation. Counted execution returns zero. Zero does not bypass an unsupported-feature check.
- **Large values.** The existing `.limit(usize)` input convention remains. Values exceeding
  `i64::MAX` retain the query builder's construction-time panic; no truncation or negative SQL
  limit.
  A representable value above a backend restriction surfaces as the backend's normal error.
- **Fewer matches.** Delete only the available matches. A short batch can result from concurrent
  deletion, predicate rechecks, or trigger suppression; do not refill it through extra statements.
- **Predicate semantics.** Preserve the original predicate's native null and comparison rules.
  A limited key-selection rewrite also retains the original predicate on the outer delete, so it
  does not become an unconditional delete of keys read earlier.
- **Concurrency.** The SQL bound still holds under concurrent writers, but row eligibility and
  conflict behavior follow native isolation. A row can disappear or stop matching; a transaction
  can fail. Ordering supplies no work-claim, skip-locked, or exactly-once guarantee.
- **Composite keys.** Identify candidates using every primary-key component, including a partition
  component. Bounding or deleting by only one component could remove more than `n` rows. Do not rely
  on SQLite rowid or PostgreSQL ctid, which are not portable model identities.
- **Relations and physical writes.** A limit on parent rows is not a limit on their children. Reject
  extra Toasty writes; users must separately account for database triggers, foreign-key actions, and
  other writes in their transaction. This API is not a transaction-wide physical-write budget.
- **Conditions.** The first version rejects explicit delete conditions for bounded or counted
  deletes. Existing filter-versus-condition behavior for ordinary deletes is unchanged; a later
  extension must preserve it rather than silently treating conditions as filters.

## Driver integration

Driver authors need two capabilities, proposed as `bounded_delete` and `delete_count`, defaulting to
false. `bounded_delete` certifies the single-statement target-row bound; `delete_count` certifies
exact
direct-delete counts. Bounded `.exec()` requires the former; bounded `.exec_count()` requires both.
Unbounded `.exec_count()` requires the latter and the single-target restrictions above. Neither flag
may cause the engine to accept a query shape that the driver cannot execute faithfully.

SQL drivers continue receiving `Operation::QuerySql`; no new operation variant is required. For
counted deletes, request a count response and preserve `ExecResponse::count(u64)` through to the
public
result. For unit execution, discard the result only after successful execution. The existing count
response is the transport, not proof that every driver already reports the required semantics.

The observable statement and count contracts are:

| Backend | Bounded statement | Exact direct count |
| --- | --- | --- |
| PostgreSQL | Same-statement limited primary-key selection | DELETE command count |
| MySQL | Native single-table `DELETE ... LIMIT` | Affected rows |
| MariaDB | Native single-table `DELETE ... LIMIT` | Affected rows |
| SQLite | Limited primary-key subquery | Direct statement changes |
| Turso variants | Verified SQLite-style key subquery, or reject | Verified count, or reject |
| DynamoDB | Initially reject | Initially reject counted deletes |

### PostgreSQL

PostgreSQL has no native delete limit. A single-key statement can have this shape:

```sql
DELETE FROM runs
WHERE created_at < $1
  AND id IN (SELECT id FROM runs WHERE created_at < $1 ORDER BY created_at, id LIMIT $2);
```

Use all key columns for composite keys, quote identifiers, and parameterize predicate values. A key
must identify at most one row across the whole delete target. PostgreSQL inheritance without a
globally unique key cannot use this rewrite: inherited child tables need not share the parent's key
constraint. Reject such a target before writing rather than allowing one candidate key to delete
several rows. This shape needs no PostgreSQL-specific locking or ctid access. The native DELETE
command count measures
rows actually deleted, including any reduction caused by deletion-suppressing triggers. Database
services using this protocol still need to support the emitted SQL and existing executor behavior;
this proposal does not certify a new service dialect or transaction mode.

See the [PostgreSQL DELETE reference](https://www.postgresql.org/docs/current/sql-delete.html).

### MySQL and MariaDB

Use the native single-table form with the original predicate, optional `ORDER BY`, and `LIMIT`:

```sql
DELETE FROM runs WHERE created_at < ? ORDER BY created_at, id LIMIT ?;
```

Use the execution result's affected-row count. Do not copy PostgreSQL's self-table subquery
rewrite to
MySQL: it has restrictions on selecting from the table being deleted. Do not rely on multi-table
`DELETE ... LIMIT`; the single-target form is the common contract. Accepted filter/order expressions
must remain valid for that form or fail before writing.

See the [MySQL DELETE reference][mysql-delete] and [MariaDB DELETE reference][mariadb-delete].

[mysql-delete]: https://docs.oracle.com/cd/E17952_01/mysql-8.4-en/delete.html
[mariadb-delete]: https://mariadb.com/kb/en/delete/

### SQLite

SQLite's direct `DELETE ... LIMIT` syntax depends on `SQLITE_ENABLE_UPDATE_DELETE_LIMIT`. Do not
require that build option. Use a same-statement limited subquery over complete model primary keys,
with the original predicate retained on the outer delete, as for PostgreSQL. The mechanism also
works
for a table without rowid. Use the statement's direct affected count, which excludes auxiliary
trigger
and foreign-key changes. SQLCipher follows this SQLite contract.

See [SQLite DELETE](https://sqlite.org/lang_delete.html) and
[SQLite change counts](https://sqlite.org/c3ref/changes.html).

### Turso

Cover each driver configuration: embedded, sync, and serverless. SQLite compatibility alone does not
certify the capability, and the native SQLite delete-limit build option must not be assumed. Enable
each flag only after verifying that configuration's limited key-subquery behavior and direct count,
including composite keys and trigger effects where that configuration supports triggers. An
unverified or incompatible configuration returns `Error::unsupported_feature` before writing.

### DynamoDB

DynamoDB deletes by complete primary key, using `DeleteItem` or transaction write actions. A query's
read limit is not a delete limit, and acknowledging a delete of a missing item is not evidence that
one row was removed. `DeleteItem` can return the old item, which could support existence-sensitive
per-item counting, but it does not make independent writes one atomic SQL-style statement.
`BatchWriteItem` is non-atomic, cannot apply per-item conditions, and does not return deleted items.

Initially advertise neither capability and reject bounded or counted deletes before mutation, even
when a key lookup could find candidates. Keep existing ordinary deletes unchanged. Future support
needs its own contract for conditional per-key mutation, missing items, unique-index maintenance,
partial failures, and transaction action limits. It must also reconcile the existing
[atomic-batches design](atomic-batches.md) and
[#1051](https://github.com/tokio-rs/toasty/issues/1051). This proposal does not change those
contracts.

See [DeleteItem][ddb-delete] and [BatchWriteItem][ddb-batch].

[ddb-delete]: https://docs.aws.amazon.com/amazondynamodb/latest/APIReference/API_DeleteItem.html
[ddb-batch]: https://docs.aws.amazon.com/amazondynamodb/latest/APIReference/API_BatchWriteItem.html

### Out-of-tree drivers

Drivers that leave the capabilities disabled continue to support ordinary unbounded unit-returning
deletes and reject the new operations before writing. Adding public capability fields may require
source changes to explicit struct literals; state that migration in the implementation PR. Drivers
opting in must provide the same bound and exact count, not a best-effort estimate. Extending the
statement representation to retain selection limits and ordering may likewise require source updates
for drivers that construct or inspect delete statements directly.

## Alternatives considered

**Change `.exec()` to return `u64`.** This matches the initial sketch but changes existing result
types, including batched results. An opt-in count method preserves existing callers.

**Expose only `.delete().limit(n)`.** This avoids query-to-delete propagation, but leaves the
natural
`.limit(n).delete()` spelling prone to silently ignoring a bound. Preserving the query limit gives
it
an explicit contract; a convenience setter can be considered later.

**Select keys on the client, then delete them.** This needs a separate concurrency and transaction
contract, loses the single-statement boundary, and requires predicate rechecking. Keep it available
as an explicit application strategy rather than pretending it is the same operation.

**Require native delete-limit syntax everywhere.** PostgreSQL lacks it and SQLite makes it optional.
A key subquery provides the same target-row bound for the accepted SQL shapes without that build
flag.
MySQL and MariaDB use their native syntax rather than a less compatible self-table rewrite.

**Automatically loop or split transactions.** This hides progress and transaction boundaries from
the
retention job, and splitting an atomic operation changes its guarantee. Each execution is one batch.

**Include all physical writes in the limit.** Database triggers and relation fan-out cannot
generally
be predicted from a model limit. A transaction-wide write budget requires a separate API and design.

**Enable best-effort DynamoDB deletes immediately.** This requires progress, failure, and count
semantics different from the proposed SQL operation. Rejecting unsupported behavior makes that
choice explicit while the separate DynamoDB contract is designed.

## Open questions

- **Count-method naming. Blocking acceptance.** The default is `.exec_count()` on `Delete<()>`,
  shared by bounded and unbounded single-target deletes; maintainers may prefer another opt-in name.
- **Conservative initial scope. Blocking acceptance.** The default rejects relation expansion,
  non-local query shapes, explicit conditions, and DynamoDB bounded/count execution. Broadening that
  scope requires an equally precise bound, count, concurrency, and failure contract.
- **Turso configuration conformance. Blocking implementation.** Default to disabled capabilities
  until embedded, sync, and serverless configurations independently satisfy the contract.
- **Mutation-metadata coordination. Blocking implementation.** Align the count surface with the
  existing mutation-result-metadata roadmap work; this document specifies direct deletes only.

## Out of scope

- Implementation in this PR: it follows in a separate PR after the design merges.
- Limits on updates, generic mutation metadata, and returning deleted models: separate API work.
- Automatic retention scheduling, retries, and time budgets: the application owns its loop.
- Transaction-wide physical-write accounting: target-row limits cannot account for all side effects.
- Relation-aware, conditional, or paginated bounded deletes: initially rejected shapes need designs.
- DynamoDB per-key batching and atomic write sets: they need a separate compatible contract.
