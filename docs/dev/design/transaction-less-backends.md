# Backends without interactive transactions

## Summary

Some databases cannot hold a transaction open across calls. Toasty wraps any
plan of more than one database operation in `BEGIN`/`COMMIT`, so a driver for
such a backend has to refuse that operation — and loses eager loading, batch
creates, and cascading writes along with it, even when the backend could
perform every one of those statements.

Two capability flags let such a driver describe what it can actually offer.
`Capability::snapshot_reads` says whether a multi-statement read can be given a
consistent snapshot; a driver that says no gets its read-only plans run
unwrapped. `Capability::atomic_write_batch` says the driver commits a set of
writes handed to it together; the engine then delivers the plan's writes
through `Connection::exec_batch` instead of a transaction.

Nothing changes for the drivers Toasty ships. The flags default to the
behaviour those drivers already have.

## Motivation

Cloudflare D1 is SQLite reached over an HTTP API. `Capability::SQLITE` and the
SQLite serializer describe it exactly; only the transport differs, since every
statement is a request rather than a call on an open connection.

D1 rejects SQL transaction control outright:

> To execute a transaction, please use the `state.storage.transaction()` or
> `state.storage.transactionSync()` APIs instead of the SQL BEGIN TRANSACTION
> or SAVEPOINT statements.

`BEGIN; INSERT ...; COMMIT;` in a single request is refused the same way, so
there is no way to open a transaction from that API at all. An out-of-tree
driver written against Toasty 0.9 therefore answers `Operation::Transaction`
with `unsupported_feature`, and 628 of 1348 integration-suite tests fail — all
of them on that one rejection.

Most of what is lost does not need a transaction to be *correct*. An eager load
is two SELECTs:

```
transaction  Start { isolation: None, read_only: false, mode: Default }
query_sql    SELECT ... FROM "authors" WHERE "id" = ?1 LIMIT 1
query_sql    SELECT ... FROM "books" WHERE EXISTS (...)
transaction  Commit
```

The transaction is there for the snapshot, not for atomicity. Run those two
statements unwrapped and `include()` returns the right rows; what is given up
is that they cannot straddle a concurrent write.

Multi-statement writes do need atomicity — and D1 can provide it. Statements
sent in one request are applied atomically: send
`INSERT (3,'c'); INSERT (4,NULL);` where the second violates `NOT NULL`, and
the request fails with row 3 absent. It is the same primitive behind
`db.batch()` in the Workers binding. The capability exists; Toasty just had no
way to reach it.

## User-facing API

Nothing changes in application code. `include()`, `create!` over several
records, and cascading writes are written the same way and mean the same
thing. What changes is which backends can run them.

A user of a backend without transactions sees two differences in behaviour,
both documented by that backend's driver:

**Reads in one plan are not a snapshot.** An eager load issues its statements
separately, so a concurrent write between them can be observed:

```rust
let author = Author::filter_by_id(id)
    .include(Author::fields().books())
    .get(&mut db)
    .await?;
```

On PostgreSQL the two statements see one snapshot. On a backend that reports
`snapshot_reads: false` they do not: a book inserted between them may or may
not appear. The rows returned are always real rows — nothing is invented or
lost — but they are not necessarily a single point in time.

**`db.transaction()` still fails.** Explicit transactions are the user asking
for something the backend does not have, and that stays an error:

```rust
db.transaction(|tx| async move { /* ... */ }).await
// Error: unsupported feature: ...
```

Batch writes remain atomic. A `create!` over several records either applies
wholly or not at all, on every backend that reports `atomic_write_batch`, in
the same way the guide already promises.

## Behavior

The planner decides whether a plan needs a transaction from what the plan does
and what the driver offers:

```rust
let needs_transaction =
    self.use_transactions && db_ops > 1 && (writes || self.snapshot_reads);
```

A plan of several statements takes a transaction when it writes, or when the
driver can give its reads a snapshot. For every driver Toasty ships,
`snapshot_reads` is `true` and `writes || true` leaves the condition exactly as
it was.

When a plan does write and the driver reports `atomic_write_batch`, the engine
hands the writes over as one set instead of opening a transaction:

1. The plan's writes are prepared without being sent.
2. They are passed to `Connection::exec_batch` in plan order.
3. The responses come back in the same order and are stored in the plan's
   variables.
4. The plan's remaining actions run as usual.

The writes are hoisted ahead of the actions that sat between them. That is
sound only because a batch is submitted before any of it runs, so a batched
write must not depend on another write's output — see [Edge cases](#edge-cases).

**Error cases.** A failing batch fails the whole plan, with the driver's error.
The backend is responsible for leaving nothing behind: a driver reporting
`atomic_write_batch` promises that a batch applies wholly or not at all. A
driver that leaves the flag off never sees `exec_batch`; the default
implementation returns `unsupported_feature` and exists only to keep the trait
backward compatible.

## Edge cases

**A write that reads a variable is never batched.** A batch is submitted before
any of it runs, so a statement consuming another statement's output would not
see it. Such plans keep the streamed path, where a driver without transactions
rejects them as before. This is conservative: independent records batch, while
a plan whose second write consumes the first's returned id does not.

**A single write is not a set.** A plan with one database operation never
needed a transaction and never becomes a batch.

**Reads and writes in one plan.** The write half decides: any plan that writes
asks for a transaction (or a batch), so a read-modify-write is not silently
split.

**Nested transactions.** A plan running inside `db.transaction()` uses
savepoints, and neither flag changes that. A backend without transactions
cannot reach this case, since `db.transaction()` already failed.

## Driver integration

**Existing drivers need to do nothing.** Both flags default to the behaviour
they already have, and `exec_batch` has a default implementation. Out-of-tree
drivers built against the previous release keep compiling and behaving
identically.

A driver for a backend without transactions sets:

```rust
fn capability(&self) -> &'static Capability {
    static CAPABILITY: OnceLock<Capability> = OnceLock::new();
    CAPABILITY.get_or_init(|| Capability {
        snapshot_reads: false,
        atomic_write_batch: true,
        ..Capability::SQLITE
    })
}
```

`snapshot_reads: false` is a statement about the backend, not a preference: set
it only when the backend genuinely cannot give a multi-statement read a
snapshot. Setting it on a backend that can would quietly weaken reads that
users expect to be consistent.

`atomic_write_batch: true` is a promise. The driver implements:

```rust
async fn exec_batch(
    &mut self,
    schema: &Arc<Schema>,
    ops: Vec<Operation>,
) -> Result<Vec<ExecResponse>>;
```

It must apply every operation or none, and return one response per operation in
the order given. A driver that cannot guarantee that must leave the flag off —
the streamed path, where the engine's `BEGIN`/`COMMIT` provides atomicity, is
still correct for it.

For a SQL driver whose backend batches by request, the implementation is to
serialize each statement and send them together. Note that bind placeholders
are numbered per statement, so a transport carrying one parameter list per
request needs the values inlined.

**The instrumented driver in the integration suite forwards `exec_batch`** and
logs each statement separately, so operation-log assertions read the same
whether a driver batched or streamed its writes. Any other wrapping driver must
forward it too; missing the override silently routes to the default and fails
with `unsupported_feature`.

## Alternatives considered

**Buffer the statements in the driver and return lazily-resolved streams**, so
the driver could send the batch when it sees `Commit`. This deadlocks: the
engine awaits each statement's rows before issuing the next, and a `create!`
over two records interleaves an `eval` after each insert, both before `Commit`.
Measured with a prototype, not inferred.

**Treat transaction boundaries as no-ops in the driver and write eagerly.**
Batches would appear to work while silently dropping atomicity — a failure
partway through leaves earlier writes committed, with nothing to distinguish
that from success. A driver that abandons a guarantee it appears to provide is
worse than one that refuses.

**One flag instead of two.** A single "no transactions" flag would have to
choose whether writes proceed unwrapped or are rejected, and both answers are
wrong for some backend. Splitting reads from writes lets a driver relax the
snapshot without relaxing atomicity, which is the whole point.

**`transaction_delivery` with a `WriteSet` mode**, as proposed in
[`atomic-batches.md`](atomic-batches.md) for DynamoDB. That design covers the
same ground for the write half, and D1's multi-statement request fits `WriteSet`
too. This design keeps the delivery decision in the executor and shapes the
payload as SQL operations rather than `TransactWrite { items }`, because that
is what a SQL backend needs. **If the two should be unified under
`transaction_delivery`, that is the better outcome** — see Open questions.

## Open questions

- **Should this fold into `transaction_delivery`?** `atomic-batches.md` proposes
  that capability with `Unsupported` / `Streamed` / `WriteSet` modes for the
  same write-side problem. Unifying avoids two mechanisms for one concern.
  *Blocking acceptance* — the answer decides the shape of the write half.
- **Should a write that depends on another write be rejected rather than
  streamed?** `atomic-batches.md` rejects them at planning time for `WriteSet`
  drivers. Falling back to the streamed path is gentler but means a backend
  without transactions still fails on those plans, just later.
  *Deferrable.*
- **Should `snapshot_reads: false` be visible to users?** A user querying such
  a backend gets non-snapshot reads with nothing in the API saying so.
  *Deferrable* — the driver documents it today.

## Out of scope

- **Read consistency mechanisms** such as D1's session bookmarks. A backend may
  offer per-session read consistency without transactions; using it is a
  driver concern and does not change this design.
- **Interactive transactions on backends that lack them.** `db.transaction()`
  continues to fail; nothing here emulates it.
- **Batch size limits.** `atomic-batches.md` covers DynamoDB's 100-action cap.
  A SQL backend batching by request has its own limits, which the driver
  surfaces as its own error.
