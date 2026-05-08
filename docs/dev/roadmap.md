# Toasty Roadmap

This roadmap is the source of truth for work the maintainers have accepted as
worth doing. An item being listed means "we agree this should exist" — not
that it's scheduled, assigned, or imminent.

Not every change belongs here. Bug fixes, small tweaks, and internal cleanup
stay as issues. A roadmap entry is for work substantial enough to warrant
visibility.

Items within a section are ordered by priority — earlier items carry more
weight, but the ordering is a signal, not a commitment.

Items may link to a GitHub issue for discussion, a design document for
detail, or both. An item with no link is a placeholder; open an issue before
starting substantial work.

To propose a new roadmap item, open an issue. If maintainers agree it fits,
the entry lands here.

## Schema & Types

- Composite keys — full support across drivers ([design](design/composite-keys.md))
- Embedded structs and enums ([design](design/enums-and-embedded-structs.md), [impl](design/embedded-enums-data-carrying-impl.md))
- `BelongsTo` fields in embedded structs ([#670])
- Native PostgreSQL enum types ([#641])
- Migrations for enum representation changes ([#724])
- Serde-serialized fields (JSON/JSONB for arbitrary Rust types) ([design](design/serialize-fields.md), [#672])
- Document and collection fields — `Vec`, `HashSet`, `HashMap`, with backend-chosen storage and a `#[document]` override ([design](design/document-fields.md))
- Foreign key constraints ([#366])
- Server-side check constraints ([#644])
- Database-side column defaults ([#642])
- Composite unique constraints ([#639])
- Partial / conditional unique constraints ([#640])
- PostgreSQL dynamic index types — GIN, GiST, BRIN, HASH ([#673])
- Partial model loading via `#[deferred]` / `Deferred<T>` ([design](design/deferred-fields.md))

[#366]: https://github.com/tokio-rs/toasty/issues/366
[#639]: https://github.com/tokio-rs/toasty/issues/639
[#640]: https://github.com/tokio-rs/toasty/issues/640
[#641]: https://github.com/tokio-rs/toasty/issues/641
[#642]: https://github.com/tokio-rs/toasty/issues/642
[#644]: https://github.com/tokio-rs/toasty/issues/644
[#670]: https://github.com/tokio-rs/toasty/issues/670
[#672]: https://github.com/tokio-rs/toasty/issues/672
[#673]: https://github.com/tokio-rs/toasty/issues/673
[#724]: https://github.com/tokio-rs/toasty/issues/724

## Query Engine

- String predicates — `contains`, `ends_with`, case-insensitive matching, regex ([#774])
- Range and set predicates — `NOT IN`, `BETWEEN`, `IS DISTINCT FROM`
- Relation filtering — filter by fields on an associated model
- Field-to-field comparison
- Arithmetic in predicates (add, subtract, multiply, divide, modulo)
- Conditional expressions — `CASE WHEN ... THEN ... ELSE ... END`
- `DISTINCT` / `DISTINCT ON` ([#425])
- Aggregates — `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`, `GROUP BY`, `HAVING` ([#421])
- Subquery comparisons (`ALL` / `ANY` / `SOME`)
- Recursive queries / CTEs ([#420])
- Lateral joins ([#419])
- Full-text search ([#423])
- Document field queries — path predicates, sub-document containment, collection operators ([design](design/document-fields.md))
- Dynamic / conditional query building — optional-filter pattern (SeaORM-style `Condition::add_option`, Diesel `BoxableExpression`)
- Query ordering & limits — multi-column `.then_by()`, direct `.limit()`, `.last()`
- Pagination with complex ORDER BY expressions ([#723])
- Backward pagination as a driver capability ([#732])
- Streaming query results — `.all()` returns a `Stream` for large result sets ([#324])
- Post-lowering optimization pass
- Pre-compiled query plans — parameterized plans that skip re-planning on repeated calls ([#266])
- Query result caching — cache results for repeated identical queries

[#266]: https://github.com/tokio-rs/toasty/issues/266
[#324]: https://github.com/tokio-rs/toasty/issues/324
[#419]: https://github.com/tokio-rs/toasty/issues/419
[#420]: https://github.com/tokio-rs/toasty/issues/420
[#421]: https://github.com/tokio-rs/toasty/issues/421
[#423]: https://github.com/tokio-rs/toasty/issues/423
[#425]: https://github.com/tokio-rs/toasty/issues/425
[#723]: https://github.com/tokio-rs/toasty/issues/723
[#732]: https://github.com/tokio-rs/toasty/issues/732
[#774]: https://github.com/tokio-rs/toasty/issues/774

## Relationships

- Many-to-many
- Polymorphic associations
- Nested preloading — multi-level `.include()`

## Data Modification

- Upsert ([#422])
- Mutation result metadata — affected row counts and "matched but unchanged" detection
- Bulk insert / update
- Database-side function expressions in create/update — `now()`, `COALESCE`, `LOWER`, `UPPER`, `LENGTH`
- Soft deletion — tombstone semantics with transparent query filtering ([#462])

[#422]: https://github.com/tokio-rs/toasty/issues/422
[#462]: https://github.com/tokio-rs/toasty/issues/462

## Transactions

- Cross-database atomic batch API — type-safe, all-or-nothing across SQL and NoSQL
- Manual SQL transactions — `BEGIN` / `COMMIT` / `ROLLBACK`, savepoints, isolation levels
- Row-level locking — `SELECT ... FOR UPDATE` / `SKIP LOCKED` ([#424])

[#424]: https://github.com/tokio-rs/toasty/issues/424

## Migrations

- Schema migration system ([#190])
- `toasty-cli` for schema management ([#190])
- Schema lock file for tracking applied migrations ([#136])

[#136]: https://github.com/tokio-rs/toasty/issues/136
[#190]: https://github.com/tokio-rs/toasty/issues/190

## Drivers

- DynamoDB Scan support ([design](design/ddb-scan.md), [#741])
- Raw SQL escape hatch ([#93])
- Connection pooling improvements ([#384])
- New driver backends
  - MongoDB — `toasty-mongodb` ([#48])
  - DuckDB ([#608])
  - MSSQL — `toasty-msql` ([#82])
  - SurrealDB ([#669])
  - libsql SQLite variant ([#78])

[#48]: https://github.com/tokio-rs/toasty/issues/48
[#78]: https://github.com/tokio-rs/toasty/issues/78
[#82]: https://github.com/tokio-rs/toasty/issues/82
[#93]: https://github.com/tokio-rs/toasty/issues/93
[#384]: https://github.com/tokio-rs/toasty/issues/384
[#608]: https://github.com/tokio-rs/toasty/issues/608
[#669]: https://github.com/tokio-rs/toasty/issues/669
[#741]: https://github.com/tokio-rs/toasty/issues/741

## Macros

- `toasty::query!()` — succinct query syntax ([design](design/query-macro.md), [#808])
- `toasty::create!()` — concise record creation ([design](design/static-assertions-create-macro.md))
- `toasty::update!()` — concise updates
- Derive macro for populating a struct from a query result ([#828])

[#808]: https://github.com/tokio-rs/toasty/issues/808
[#828]: https://github.com/tokio-rs/toasty/issues/828

## Runtime

- Concurrent task execution — run independent plan nodes concurrently
- Cancellation and cleanup — drop detection, transaction rollback on cancel
- Internal instrumentation — per-phase timing, planner CPU time

## Observability

- Query logging — `tracing` debug / trace output from the engine ([#254])

[#254]: https://github.com/tokio-rs/toasty/issues/254

## Safety

- `#[sensitive]` field flagging — automatic redaction in logs, traces, and errors
- Trusted vs. untrusted expression tagging — skip escaping for engine-produced values; parameterize external input ([#237])

[#237]: https://github.com/tokio-rs/toasty/issues/237
