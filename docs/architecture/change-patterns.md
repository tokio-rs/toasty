# Change Patterns

This document describes where different types of changes should be made and provides examples of implementing features across the codebase.

## Change Location Map

| Type of Change | Primary Crate | Also Update |
|----------------|---------------|-------------|
| New primitive type | toasty-core | toasty-codegen, toasty-sql, all drivers |
| Query feature (ORDER BY, LIMIT) | toasty-core | toasty-codegen, toasty, toasty-sql |
| New model attribute | toasty-codegen | toasty-core (if new schema types needed) |
| Driver operation | toasty-core | toasty-driver-* |
| SQL syntax | toasty-sql | — |
| Relationship type | toasty | toasty-core, toasty-codegen |

## Crate-Specific Patterns

### toasty-core

- New data types: update `schema/app/field/primitive.rs`
- Query features: add statement nodes in `stmt/`
- Driver operations: define in `driver/operation.rs`

### toasty-codegen

- New model features require updates to both schema parsing and expansion
- Query methods follow a builder pattern with method chaining
- Generated code must use fully qualified paths (e.g., `#toasty::`)

### toasty-driver-*

- Drivers implement the `driver::Driver` trait from toasty-core
- SQL drivers use toasty-sql for query serialization
- NoSQL drivers have custom operation implementations

### toasty-sql

- New SQL features require serializer updates
- Database-specific syntax handled via `Flavor` enum
- Parameter binding varies by database

## Cross-Crate Examples

### Adding a New Primitive Type

1. Define in `toasty-core/src/schema/app/field/primitive.rs`
2. Add to `toasty-core/src/stmt/ty.rs` and `value.rs`
3. Update codegen in `toasty-codegen/src/schema/ty.rs`
4. Add SQL serialization in `toasty-sql/src/serializer/ty.rs`
5. Implement driver conversions in each `toasty-driver-*/src/value.rs`
6. Add tests in `tests/tests/tys.rs`

### Adding a Query Feature (e.g., ORDER BY, LIMIT)

1. Add statement nodes in `toasty-core/src/stmt/`
2. Update Visit/VisitMut traits in `toasty-core/src/stmt/visit*.rs`
3. Add builder methods in `toasty-codegen/src/expand/query.rs`
4. Implement in engine pipeline (see [Query Engine Architecture](query-engine.md))
5. Add SQL serialization in `toasty-sql/src/serializer/statement.rs`
6. Write integration tests
