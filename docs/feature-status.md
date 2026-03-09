# Toasty Feature Status

Last updated: 2026-03-07

This document tracks user-facing feature status from repository evidence.

Status definitions:
- `Implemented`: usable behavior exists in current code, backed by tests and/or
  direct implementation evidence.
- `Partial`: some tested paths exist, but important gaps or explicit TODO/panic
  paths remain.
- `Not implemented`: no usable implementation yet (or only stubbed API).

Confidence percentages below represent confidence in the classification claim,
not a quality score for the feature itself.

## Implemented (Evidence-Backed)

| Feature | Status | Backend scope | Confidence | Evidence | Notes |
| --- | --- | --- | --- | --- | --- |
| Core model derive and schema attributes (`#[key]`, `#[index]`, `#[unique]`, `#[column(...)]`) | Implemented | SQLite, PostgreSQL, MySQL, DynamoDB | 95% | [`one_model_crud.rs`](../crates/toasty-driver-integration-suite/src/tests/one_model_crud.rs), [`field_column_name.rs`](../crates/toasty-driver-integration-suite/src/tests/field_column_name.rs), [`field_column_type.rs`](../crates/toasty-driver-integration-suite/src/tests/field_column_type.rs) | Core model/schema flows are exercised end-to-end. |
| Auto/default/update field behavior (`#[auto]`, `#[default]`, `#[update]`) | Implemented | All supported backends (with capability guards where needed) | 95% | [`field_auto.rs`](../crates/toasty-driver-integration-suite/src/tests/field_auto.rs), [`default_and_update.rs`](../crates/toasty-driver-integration-suite/src/tests/default_and_update.rs), [`guide/default-and-update.md`](guide/default-and-update.md) | Includes UUID auto modes and timestamp auto behavior. |
| CRUD + generated query methods (`get_by_*`, `filter_by_*`) | Implemented | SQLite, PostgreSQL, MySQL, DynamoDB | 94% | [`one_model_crud.rs`](../crates/toasty-driver-integration-suite/src/tests/one_model_crud.rs), [`one_model_query.rs`](../crates/toasty-driver-integration-suite/src/tests/one_model_query.rs) | Includes batch-get and update/delete flows. |
| Filter DSL (`and`/`or`/`not`, comparisons, nullable `is_some`/`is_none`) | Implemented | SQL + DynamoDB for tested paths; some filters are SQL-only | 88% | [`one_model_query.rs`](../crates/toasty-driver-integration-suite/src/tests/one_model_query.rs), [`one_model_option_filter.rs`](../crates/toasty-driver-integration-suite/src/tests/one_model_option_filter.rs) | Option-null filters are currently SQL-focused tests. |
| Sorting, `.limit()`, and cursor pagination (`.paginate()`, `.next()`, `.prev()`) | Implemented | SQL (integration-tested) | 92% | [`one_model_sort_limit.rs`](../crates/toasty-driver-integration-suite/src/tests/one_model_sort_limit.rs), [`guide/pagination.md`](guide/pagination.md) | `.limit()` is implemented even though older docs called it future work. |
| Relationship CRUD (`HasMany`, `BelongsTo`, `HasOne`) | Implemented | SQLite, PostgreSQL, MySQL, DynamoDB (for covered scenarios) | 90% | [`has_many_crud_basic.rs`](../crates/toasty-driver-integration-suite/src/tests/has_many_crud_basic.rs), [`has_one_crud_basic.rs`](../crates/toasty-driver-integration-suite/src/tests/has_one_crud_basic.rs), [`belongs_to_configured.rs`](../crates/toasty-driver-integration-suite/src/tests/belongs_to_configured.rs) | Covers optional/required combinations and association updates. |
| Association link/unlink and scoped relation operations | Implemented | SQLite, PostgreSQL, MySQL, DynamoDB (covered scenarios) | 90% | [`has_many_link_unlink.rs`](../crates/toasty-driver-integration-suite/src/tests/has_many_link_unlink.rs), [`has_many_scoped_query.rs`](../crates/toasty-driver-integration-suite/src/tests/has_many_scoped_query.rs) | Includes insert/remove/reassign behavior. |
| Eager loading via `.include(...)`, including nested preloads | Implemented | SQLite, PostgreSQL, MySQL, DynamoDB (covered scenarios) | 90% | [`preload.rs`](../crates/toasty-driver-integration-suite/src/tests/preload.rs) | Multi-level include paths are exercised by integration tests. |
| Interactive transactions + nested savepoints + builder options | Implemented | SQL backends | 97% | [`tx_interactive.rs`](../crates/toasty-driver-integration-suite/src/tests/tx_interactive.rs), [`tx_atomic_stmt.rs`](../crates/toasty-driver-integration-suite/src/tests/tx_atomic_stmt.rs), [`guide/transactions.md`](guide/transactions.md) | DynamoDB transactions are not supported in current user guide. |
| Batch query API (`toasty::batch`) | Implemented | SQL backends | 96% | [`batch_query.rs`](../crates/toasty-driver-integration-suite/src/tests/batch_query.rs), [`lib.rs`](../crates/toasty/src/lib.rs) | Tuple batch query composition is tested. |
| `toasty::create!` macro | Implemented | SQLite, PostgreSQL, MySQL, DynamoDB (covered scenarios) | 97% | [`create_macro.rs`](../crates/toasty-driver-integration-suite/src/tests/create_macro.rs), [`lib.rs`](../crates/toasty-macros/src/lib.rs) | Includes nested associations and batch create forms. |
| Embedded structs and enums (`#[derive(toasty::Embed)]`) including data-carrying enums | Implemented | SQLite, PostgreSQL, MySQL, DynamoDB (covered scenarios) | 91% | [`embedded_struct.rs`](../crates/toasty-driver-integration-suite/src/tests/embedded_struct.rs), [`embedded_enum_unit.rs`](../crates/toasty-driver-integration-suite/src/tests/embedded_enum_unit.rs), [`embedded_enum_data.rs`](../crates/toasty-driver-integration-suite/src/tests/embedded_enum_data.rs) | Includes query and update paths for embedded fields. |
| Serialized JSON fields (`#[serialize(json)]`) | Implemented | SQLite, PostgreSQL, MySQL, DynamoDB (covered scenarios) | 94% | [`serialize.rs`](../crates/toasty-driver-integration-suite/src/tests/serialize.rs) | Includes nullable and non-nullable Option behavior. |
| Jiff temporal types | Implemented | SQLite, PostgreSQL, MySQL, DynamoDB | 95% | [`jiff.rs`](../crates/toasty-driver-integration-suite/src/tests/jiff.rs), [`guide/jiff.md`](guide/jiff.md) | Includes precision/storage behavior across backends. |
| Decimal and BigDecimal support | Implemented | Backend-dependent capability matrix | 90% | [`decimal.rs`](../crates/toasty-driver-integration-suite/src/tests/decimal.rs), [`bigdecimal.rs`](../crates/toasty-driver-integration-suite/src/tests/bigdecimal.rs) | Numeric precision modes vary by backend capability flags. |
| Composite-key tested workflows | Implemented | SQL + DynamoDB (covered paths) | 82% | [`one_model_composite_key.rs`](../crates/toasty-driver-integration-suite/src/tests/one_model_composite_key.rs), [`one_model_partitioned_crud.rs`](../crates/toasty-driver-integration-suite/src/tests/one_model_partitioned_crud.rs), [`one_model_query.rs`](../crates/toasty-driver-integration-suite/src/tests/one_model_query.rs) | See partial section for known composite-key gaps. |
| Migration CLI (`generate`, `apply`, `snapshot`, `drop`, `reset`) | Implemented | SQL backends | 93% | [`migration.rs`](../crates/toasty-cli/src/migration.rs), [`generate.rs`](../crates/toasty-cli/src/migration/generate.rs), [`apply.rs`](../crates/toasty-cli/src/migration/apply.rs), [`reset.rs`](../crates/toasty-cli/src/migration/reset.rs) | CLI command implementation is present; DynamoDB migrations are still not implemented. |
| `Db::reset_db()` runtime reset path | Implemented | Backends with reset support in tests | 95% | [`reset_db.rs`](../crates/toasty-driver-integration-suite/src/tests/reset_db.rs) | Used by integration tests and CLI reset flow. |

## Remaining Work (Partial or Missing)

### Partial

| Feature | Status | Confidence | Evidence | Notes |
| --- | --- | --- | --- | --- |
| Full composite-key parity across engine/relation optimization and DynamoDB batch edge cases | Partial | 97% | [`roadmap/composite-keys.md`](roadmap/composite-keys.md), [`lift_in_subquery.rs`](../crates/toasty/src/engine/simplify/lift_in_subquery.rs), [`delete_by_key.rs`](../crates/toasty-driver-dynamodb/src/op/delete_by_key.rs) | Core paths are tested, but documented TODO/panic paths remain. |
| Multi-column ordering convenience via `.then_by()` | Partial | 95% | [`guide/pagination.md`](guide/pagination.md), [`one_model_sort_limit.rs`](../crates/toasty-driver-integration-suite/src/tests/one_model_sort_limit.rs) | Manual multi-column ordering works, chain convenience method is pending. |
| Some DynamoDB query/index rewrite edge cases | Partial | 83% | [`or_rewrite.rs`](../crates/toasty/src/engine/index/or_rewrite.rs), [`index_match.rs`](../crates/toasty/src/engine/index/index_match.rs) | Primary tested scenarios work; some branch shapes remain TODO. |

### Not Implemented

| Feature | Status | Confidence | Evidence | Notes |
| --- | --- | --- | --- | --- |
| `toasty::query!` macro | Not implemented | 99% | [`lib.rs`](../crates/toasty-macros/src/lib.rs) | Macro exists but currently expands to a placeholder print. |
| `include_schema!` macro | Not implemented | 99% | [`lib.rs`](../crates/toasty-macros/src/lib.rs) | Macro is explicitly `todo!()`. |
| `toasty::update!` macro | Not implemented | 95% | [`roadmap/README.md`](roadmap/README.md), [`lib.rs`](../crates/toasty/src/lib.rs), [`lib.rs`](../crates/toasty-macros/src/lib.rs) | Listed as roadmap item, not exported/implemented in current macros. |
| Many-to-many relationships | Not implemented | 90% | [`roadmap/README.md`](roadmap/README.md) | Still listed as future relationship work. |
| Polymorphic associations | Not implemented | 92% | [`roadmap/README.md`](roadmap/README.md) | Still listed as future relationship work. |
| Deferred field loading (`#[deferred]`, `Deferred<T>`) | Not implemented | 90% | [`roadmap/README.md`](roadmap/README.md) | Described as planned partial loading feature. |
| Upsert API | Not implemented | 92% | [`roadmap/README.md`](roadmap/README.md) | Listed under data modification roadmap. |
| Raw SQL user escape hatch | Not implemented | 90% | [`roadmap/README.md`](roadmap/README.md) | Listed under query building roadmap. |
| DynamoDB migrations | Not implemented | 99% | [`lib.rs`](../crates/toasty-driver-dynamodb/src/lib.rs) | Driver migration generation/apply paths are `todo!()`. |
| Cassandra driver | Not implemented | 96% | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Current architecture lists SQLite/PostgreSQL/MySQL/DynamoDB drivers only. |
