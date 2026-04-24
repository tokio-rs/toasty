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
  - Composite foreign key optimization in query simplification
  - Composite PK in expression rewriting and `IN`-list operations
  - `HasMany` / `BelongsTo` with composite foreign keys
  - Junction tables / many-to-many with composite keys
  - DynamoDB: batch delete/update, composite unique indexes
- Embedded structs and enums ([design](design/enums-and-embedded-structs.md), [impl](design/embedded-enums-data-carrying-impl.md))
  - Tuple variants
  - Shared columns across variants
  - Partial updates within a variant
  - DynamoDB encoding for data-carrying enum variants
- Native PostgreSQL enum types
  - Migrations for enum representation changes ([#724])
- Serde-serialized fields (JSON/JSONB for arbitrary Rust types) ([design](design/serialize-fields.md))
- Embedded collections (arrays, maps, sets)
  - Array membership / containment predicates (`has`, `has_every`, `has_some`)
- Partial model loading via `#[deferred]` / `Deferred<T>`

[#724]: https://github.com/tokio-rs/toasty/issues/724

## Query Engine

- String predicates
  - `contains` / `starts_with` / `ends_with`
  - `LIKE` with escape handling
  - Case-insensitive matching
  - Regex matching (`regex` / `iregex`)
- Range and set predicates
  - `NOT IN`
  - `BETWEEN` / range queries
  - `IS DISTINCT FROM` — NULL-safe inequality
- Relation filtering — filter by fields on an associated model
- Field-to-field comparison
- Arithmetic in predicates (add, subtract, multiply, divide, modulo)
- Conditional expressions — `CASE WHEN ... THEN ... ELSE ... END`
- Aggregates
  - `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`
  - `GROUP BY` / `HAVING`
- Subquery improvements
  - Subquery comparisons (`ALL` / `ANY` / `SOME`)
- Full-text search
  - User-facing builder API
  - PostgreSQL `tsvector` / `tsquery`
  - MySQL `FULLTEXT` / `MATCH ... AGAINST`
  - SQLite FTS5 integration
- JSON field queries
  - Core AST + path-traversal user API
  - Per-backend SQL serialization (PostgreSQL `jsonb`, MySQL JSON, SQLite `json_extract`)
- Dynamic / conditional query building — optional-filter pattern (SeaORM-style `Condition::add_option`, Diesel `BoxableExpression`)
- Query ordering & limits
  - Multi-column `.then_by()`
  - Direct `.limit()` for non-paginated queries
  - `.last()` convenience
  - Pagination with complex ORDER BY expressions (non-column references) ([#723])
- Post-lowering optimization pass
  - Single-pass predicate analysis (not per-node)
  - Equivalence classes for transitive constraints
  - Structured constraint representation (constants, ranges, exclusions)
  - Targeted normalization without full DNF
  - `ExprLet` inlining — move from `lower_returning` into the post-lowering pass
- Pre-compiled query plans — parameterized plans that skip re-planning on repeated calls
- Query result caching — cache results for repeated identical queries

[#723]: https://github.com/tokio-rs/toasty/issues/723

## Relationships

- Many-to-many
- Polymorphic associations
- Nested preloading — multi-level `.include()`

## Data Modification

- Upsert
  - SQL: `ON CONFLICT` (PostgreSQL/SQLite), `ON DUPLICATE KEY UPDATE` (MySQL), `MERGE`
  - Insert-or-ignore (`DO NOTHING` / `INSERT IGNORE`)
  - Conflict target by column, constraint name, or partial index
  - Column update control (all / subset / raw expression)
  - `EXCLUDED` pseudo-table access in update expressions
  - Bulk multi-row upsert
  - DynamoDB: `PutItem` vs. conditional `UpdateItem`
- Mutation result metadata
  - Affected row counts for update and delete
  - Distinguish "no rows matched" from "matched but unchanged"
- Bulk insert / update
- Database-side function expressions in create/update
  - `toasty::stmt::now()` → `NOW()` / `CURRENT_TIMESTAMP` / `datetime('now')`
  - Future scalar functions: `COALESCE`, `LOWER`, `UPPER`, `LENGTH`

## Transactions

- Cross-database atomic batch API
  - Works across SQL and NoSQL
  - Type-safe operation batching
  - All-or-nothing semantics
- Manual SQL transactions
  - `BEGIN` / `COMMIT` / `ROLLBACK`
  - Savepoints and nested transactions
  - Isolation-level configuration

## Migrations

- Schema migration system
  - Migration generation from schema diffs
  - Rollback support
  - Schema versioning
- `toasty-cli` for schema management

## Drivers

- Raw SQL escape hatch
  - Arbitrary SQL statements
  - Parameterized queries with type-safe bindings
  - Raw fragments inside typed queries
- Connection pooling improvements

## Macros

- `toasty::query!()` — succinct query syntax ([design](design/query-macro.md))
- `toasty::create!()` — concise record creation ([design](design/static-assertions-create-macro.md))
- `toasty::update!()` — concise updates

## Runtime

- Concurrent task execution
  - In-flight task manager (replaces the ad-hoc background task)
  - Run independent plan nodes concurrently
- Cancellation and cleanup
  - Detect dropped completion futures
  - Roll back incomplete transactions on cancel
  - Release resources without orphaned state
- Internal instrumentation
  - Per-phase timing (planning, simplify, exec, serialization)
  - Planner CPU time to surface expensive plans

## Observability

- Query logging

## Safety

- `#[sensitive]` field flagging — automatic redaction in logs, traces, and errors
- Trusted vs. untrusted expression tagging — skip escaping for engine-produced values; parameterize external input
