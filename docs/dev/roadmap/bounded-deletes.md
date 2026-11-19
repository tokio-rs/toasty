# Bounded deletes

A retention job on a database with a per-transaction row-modification ceiling needs an explicit
bound on each delete and an accurate progress count. Preserve a model query's `.limit(n)` when it
becomes a delete, and add opt-in `.exec_count()` without changing unit-returning `.exec()`.

Discussion: [#1328](https://github.com/tokio-rs/toasty/issues/1328).
Contract: [bounded-delete design](../design/bounded-deletes.md).

Completion requires:

- A single-statement limit on direct target-model rows; never an unbounded fallback or implicit
  loop.
- Exact direct-delete counts, zero-limit behavior, complete composite keys, and retained predicates.
- Explicit subset ordering, native concurrency behavior, and rejection before writing of unsupported
  query shapes or plans requiring additional Toasty writes.
- PostgreSQL key selection, native MySQL/MariaDB limits, and SQLite key selection without requiring
  `SQLITE_ENABLE_UPDATE_DELETE_LIMIT`; Turso capabilities verified per driver configuration.
- Capability errors for DynamoDB until a separate compatible per-key contract is accepted.
- Driver conformance coverage for bounds, counts, empty matches, composite keys, concurrency, and
  capability rejection; an ordinary unbounded delete must retain its existing API result.

The limit is not a transaction-wide budget for triggers, foreign-key actions, or other writes. Each
batch needs its own commit when the purpose is to stay under a transaction ceiling. Implementation
follows only after the roadmap and design are accepted.
