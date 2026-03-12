# Toasty Feature Overview

This page centralizes the guide map and current feature-status summary for
users evaluating Toasty.

## Documentation Map

Start with:

- Full feature matrix (implemented, partial, missing):
  [feature-status.md](feature-status.md)

Implemented feature guides:

- [modeling-and-querying-basics.md](guide/modeling-and-querying-basics.md)
- [relationships-loading-transactions-batch.md](guide/relationships-loading-transactions-batch.md)
- [macros-embedded-serialized-and-numeric-types.md](guide/macros-embedded-serialized-and-numeric-types.md)
- [implemented-advanced-patterns.md](guide/implemented-advanced-patterns.md)
- [composite-keys-migrations-and-known-gaps.md](guide/composite-keys-migrations-and-known-gaps.md)

Known-gap guides:

- [gaps-query-macros-and-many-to-many.md](guide/gaps-query-macros-and-many-to-many.md)
- [gaps-polymorphic-deferred-upsert-raw-sql-and-dynamodb-migrations.md](guide/gaps-polymorphic-deferred-upsert-raw-sql-and-dynamodb-migrations.md)
- [gaps-cassandra-driver.md](guide/gaps-cassandra-driver.md)

## Feature Snapshot

For per-item evidence and confidence details, use
[feature-status.md](feature-status.md).

### What Works Today (Implemented and Well-Exercised)

- Modeling and schema attributes: `#[derive(toasty::Model)]`, `#[key]`,
  `#[index]`, `#[unique]`, `#[column(...)]`, `#[auto]`, `#[default]`,
  `#[update]`.
- Querying: generated `filter_by_*` methods, field DSL filters, logical
  composition (`and`, `or`, `not`), nullable filters (`is_some`, `is_none`),
  sorting, `.limit()`, and cursor pagination.
- Relationships: `HasMany`, `BelongsTo`, and `HasOne` CRUD flows, scoped
  queries, link/unlink operations, and eager loading with `.include(...)`
  including nested preloads.
- Transactions (SQL): interactive transactions, nested savepoints, rollback on
  drop, and transaction builder controls (isolation level and read-only mode).
- Data types and field encodings: primitive types, UUIDs, jiff time types,
  `rust_decimal::Decimal`, `bigdecimal::BigDecimal`, embedded structs/enums, and
  `#[serialize(json)]` fields.
- Composite-key workflows: tested paths for batch get by composite key, and
  partition/local key update/delete/query behavior.
- Batch and macros: `toasty::batch(...)` and `toasty::create!(...)`.
- Schema management: migration CLI commands (`generate`, `apply`, `snapshot`,
  `drop`, `reset`) for SQL backends.

### What's Partial or Missing

- Composite-key parity is still partial in some engine and DynamoDB paths.
- `toasty::query!` and `include_schema!` macros are currently stubs.
- `toasty::update!` macro is not implemented.
- `.then_by()` convenience ordering is not implemented (manual multi-column
  ordering works via `OrderBy::from([...])`).
- Many-to-many relations, polymorphic associations, deferred fields, upsert,
  and raw SQL escape-hatch APIs are still roadmap items.
- DynamoDB migration generation/apply support is not implemented.
- Cassandra driver support is not implemented.
