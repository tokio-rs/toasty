# Backends without interactive transactions

## Summary

Some databases cannot hold a transaction open across calls. Toasty wraps any
plan of more than one database operation in `BEGIN`/`COMMIT`, so a driver for
such a backend has to refuse that operation — and loses eager loading along
with it, even when the backend could run every statement in the plan.

`Capability::interactive_transactions` lets a driver say it cannot. A driver
that reports `false` gets its read-only plans run unwrapped, because the
transaction those plans take is there for the snapshot rather than for
atomicity. Writes are untouched by this design: a plan that writes still asks
for a transaction, and making that work on such a backend is
[`atomic-batches.md`](atomic-batches.md)'s `transaction_delivery`.

Nothing changes for the drivers Toasty ships.

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

Much of what is lost does not need a transaction to be *correct*. An eager load
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

Multi-statement *writes* are a different problem with a different answer, and
[`atomic-batches.md`](atomic-batches.md) is already working it: a backend that
cannot open a transaction may still commit a set of writes submitted together,
which is what `transaction_delivery` names. This design does not touch them.

## User-facing API

Nothing changes in application code. `include()` is written the same way and
means the same thing. What changes is which backends can run it.

A user of a backend without interactive transactions sees two differences in
behaviour, both documented by that backend's driver:

**Reads in one plan are not a snapshot.** An eager load issues its statements
separately, so a concurrent write between them can be observed:

```rust
let author = Author::filter_by_id(id)
    .include(Author::fields().books())
    .get(&mut db)
    .await?;
```

On PostgreSQL the two statements see one snapshot. On a backend that reports
`interactive_transactions: false` they do not: a book inserted between them may
or may not appear. The rows returned are always real rows — nothing is invented
or lost — but they are not necessarily a single point in time.

**`db.transaction()` still fails.** An explicit transaction is the user asking
for something the backend does not have, and that stays an error:

```rust
db.transaction(|tx| async move { /* ... */ }).await
// Error: unsupported feature: ...
```

A plan that writes more than once also still fails on such a backend, exactly
as it does today. Changing that is `transaction_delivery`'s job, not this
flag's.

## Behavior

The planner decides whether a plan needs a transaction from what the plan does
and what the driver offers:

```rust
let needs_transaction =
    self.use_transactions && db_ops > 1 && (writes || self.interactive_transactions);
```

A plan of several statements takes a transaction when it writes, or when the
driver has transactions to give its reads a snapshot with. For every SQL driver
Toasty ships, `interactive_transactions` is `true`, and `writes || true` leaves
the condition exactly as it was.

`use_transactions` — today `capability().sql` — is deliberately left in place
rather than folded into the new flag. It is what keeps a non-SQL driver out of
this path entirely: `Capability::DYNAMODB` reports
`interactive_transactions: false` truthfully, but with `sql: false` the
condition short-circuits before the new flag is read, so DynamoDB's behaviour
is unchanged. Replacing `use_transactions` with `interactive_transactions`
would start demanding transactions of multi-write DynamoDB plans that do not
take them today.

**Error cases.** None are added. A read-only plan that previously failed on
`Operation::Transaction` now runs; everything else fails where it failed
before.

## Edge cases

**A single operation is unaffected.** A plan with one database operation never
needed a transaction and still does not.

**Reads and writes in one plan.** The write decides: any plan that writes asks
for a transaction, so a read-modify-write is never silently split.

**Nested transactions.** A plan running inside `db.transaction()` uses
savepoints, and this flag does not change that. A backend without interactive
transactions cannot reach the case, since `db.transaction()` already failed.

## Driver integration

**Existing drivers need to do nothing.** The flag defaults to the behaviour
they already have. Out-of-tree drivers built against the previous release keep
compiling and behaving identically.

A driver for a backend that cannot hold a transaction open sets:

```rust
fn capability(&self) -> &'static Capability {
    static CAPABILITY: OnceLock<Capability> = OnceLock::new();
    CAPABILITY.get_or_init(|| Capability {
        interactive_transactions: false,
        ..Capability::SQLITE
    })
}
```

`interactive_transactions: false` is a statement about the backend, not a
preference: set it only when the backend genuinely cannot open a transaction.
Setting it on a backend that can would quietly weaken reads that users expect
to be consistent.

## Alternatives considered

**Name the flag for the property the engine wants — `snapshot_reads` — rather
than the mechanism the backend lacks.** The condition then reads as intent
("take a transaction if the driver can give these reads a snapshot"). It was
the first spelling of this design. `interactive_transactions` won because every
other field of `Capability` describes a mechanism the backend either has or
does not, and because a driver author can answer it without reasoning about
what the engine will infer. A backend offering snapshot reads *without*
interactive transactions would make the two diverge; none of Toasty's drivers
is one, and such a backend's read consistency is a driver concern either way
(see Out of scope).

**A single "no transactions" flag covering reads and writes.** It would have to
choose whether writes proceed unwrapped or are rejected, and both answers are
wrong for some backend. Writes proceeding unwrapped silently drops atomicity —
a failure partway through leaves earlier writes committed, indistinguishable
from success. Keeping the read relaxation separate from the write delivery lets
a driver relax the snapshot without relaxing atomicity, and lets each half be
reviewed against the backend behaviour it actually depends on.

**Let the driver treat transaction boundaries as no-ops.** A driver could
accept `Start`/`Commit` and do nothing, and read-only plans would work with no
engine change at all. It also makes every multi-write plan appear to work while
dropping atomicity, with nothing to distinguish a partial failure from success.
A driver that abandons a guarantee it appears to provide is worse than one that
refuses.

## Open questions

- **Should `interactive_transactions: false` be visible to users?** A user
  querying such a backend gets non-snapshot reads with nothing in the API
  saying so. *Deferrable* — the driver documents it today.

## Out of scope

- **Atomic multi-write delivery.** [`atomic-batches.md`](atomic-batches.md)
  covers it with `transaction_delivery`; D1's multi-statement request is a
  `WriteSet`-shaped primitive and belongs in that design rather than this one.
  Until it lands, a multi-write plan still fails on a backend without
  interactive transactions.
- **Read consistency mechanisms** such as D1's session bookmarks. A backend may
  offer per-session read consistency without transactions; using it is a driver
  concern and does not change this design.
- **Emulating interactive transactions.** `db.transaction()` continues to fail;
  nothing here stands in for it.
