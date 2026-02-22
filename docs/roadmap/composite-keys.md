# Composite Key Support

## Overview

Toasty has partial composite key support. Basic CRUD operations work for models with composite primary keys (both field-level `#[key]` and model-level `#[key(partition = ..., local = ...)]`), but several engine optimizations, relationship patterns, and driver operations panic or fall back when encountering composite keys.

This document catalogs the gaps, surveys how other ORMs handle composite keys, identifies common SQL patterns that require composite key support, and proposes a phased implementation plan.

## Current State

### What Works

**Schema definition** — Two syntaxes for composite keys:

```rust
// Field-level: multiple #[key] attributes
#[derive(Debug, toasty::Model)]
struct Foo {
    #[key]
    one: String,
    #[key]
    two: String,
}

// Model-level: partition/local keys (designed for DynamoDB compatibility)
#[derive(Debug, toasty::Model)]
#[key(partition = user_id, local = id)]
struct Todo {
    #[auto]
    id: uuid::Uuid,
    user_id: Id<User>,
    title: String,
}
```

**Generated query methods** for composite keys:
- `filter_by_<field1>_and_<field2>()` — filter by both key fields
- `get_by_<field1>_and_<field2>()` — get a single record by both keys
- `filter_by_<field1>_and_<field2>_batch()` — batch get by key tuples
- `filter_by_<partition_field>()` — filter by partition key alone
- Comparison operators on local keys: `gt()`, `ge()`, `lt()`, `le()`, `ne()`, `eq()`

**Database support:**
- SQL databases (SQLite, PostgreSQL, MySQL): composite primary keys via field-level `#[key]`
- DynamoDB: partition/local key syntax (max 2 keys: 1 partition + 1 local)

**Test coverage:**
- `one_model_composite_key::batch_get_by_key` — basic CRUD with field-level composite keys
- `one_model_query` — partition/local key queries with range operators
- `has_many_crud_basic::has_many_when_fk_is_composite` — HasMany with composite FK (working)
- `embedded` — composite keys with embedded struct fields
- `examples/composite-key/` — end-to-end example application

### What Does Not Work

The following locations contain `todo!()`, `assert!()`, or `panic!()` that block composite key usage:

#### Engine Simplification (5 locations)

| File | Line | Issue |
|------|------|-------|
| `engine/simplify/expr_binary_op.rs` | 23-25 | `todo!("handle composite keys")` when simplifying equality on model references with composite PKs |
| `engine/simplify/expr_binary_op.rs` | 43-45 | `todo!("handle composite keys")` when simplifying binary ops on composite FK fields |
| `engine/simplify/expr_in_list.rs` | 30-32 | `todo!()` when optimizing IN-list expressions for models with composite PKs |
| `engine/simplify/lift_in_subquery.rs` | 92-96 | `assert_eq!(len, 1, "TODO: composite keys")` — subquery lifting restricted to single-field FKs |
| `engine/simplify/lift_in_subquery.rs` | 109-111, 145-148, 154-157 | Three more `todo!("composite keys")` in BelongsTo and HasOne subquery lifting |
| `engine/simplify/rewrite_root_path_expr.rs` | 18-19 | `todo!("composite primary keys")` when rewriting path expressions with key constraints |

#### Engine Lowering (2 locations)

| File | Line | Issue |
|------|------|-------|
| `engine/lower/insert.rs` | 90-92 | `todo!()` when lowering inserts with BelongsTo relations that have composite FKs |
| `engine/lower.rs` | 893-896 | Unhandled else branch when lowering relationships with composite FKs |

#### DynamoDB Driver (4 locations)

| File | Line | Issue |
|------|------|-------|
| `driver-dynamodb/op/update_by_key.rs` | 197 | `assert!(op.keys.len() == 1)` — batch update limited to single key |
| `driver-dynamodb/op/delete_by_key.rs` | 119-121 | `panic!("only 1 key supported so far")` — batch delete limited to single key |
| `driver-dynamodb/op/delete_by_key.rs` | 33 | `panic!("TODO: support more than 1 unique index")` |
| `driver-dynamodb/op/create_table.rs` | 113 | `assert_eq!(1, index.columns.len())` — composite unique indexes unsupported |

#### Stubbed Tests (2 tests)

| File | Test | Status |
|------|------|--------|
| `has_many_crud_basic.rs` | `has_many_when_pk_is_composite` | Empty — not implemented |
| `has_many_crud_basic.rs` | `has_many_when_fk_and_pk_are_composite` | Empty — not implemented |

### Design Constraints

- **Auto-increment is intentionally forbidden with composite keys.** The schema verifier rejects `#[auto(increment)]` on composite PK tables. UUID auto-generation is the supported alternative.
- **DynamoDB limits composite keys to 2 columns** (1 partition + 1 local). This is a DynamoDB limitation, not a Toasty limitation.

## How Other ORMs Handle Composite Keys

### Rust ORMs

**Diesel** — First-class composite key support. `#[diesel(primary_key(col1, col2))]` on the struct; `find()` accepts a tuple `(val1, val2)`; `Identifiable` returns a tuple reference. BelongsTo works with composite keys via explicit `foreign_key` attribute. Compile-time type checking through generated code.

**SeaORM** — Supports composite keys via multiple `#[sea_orm(primary_key)]` field attributes. `PrimaryKeyTrait::ValueType` is a tuple. `find_by_id()` and `delete_by_id()` accept tuples. DAO pattern works fully. Composite foreign keys are less ergonomic but functional.

### Python ORMs

**SQLAlchemy** — Gold standard for composite key support. Multiple `primary_key=True` columns define a composite PK. `session.get(Model, (a, b))` for lookups. `ForeignKeyConstraint` at the table level handles composite FKs cleanly. Identity map uses tuples. All features (eager/lazy loading, cascades, relationships) work uniformly with composite keys.

**Django** — Added `CompositePrimaryKey` in Django 5.2 (2025) after years of surrogate-key-only design. `pk` returns a tuple. `Model.objects.get(pk=(1, 2))` works. Composite FK support is still limited. Ecosystem (admin, REST frameworks, third-party packages) is catching up.

**Tortoise ORM** — No composite PK support. Surrogate key + unique constraint is the only option.

### JavaScript/TypeScript ORMs

**Prisma** — `@@id([field1, field2])` defines composite PKs. Auto-generates compound field names (`field1_field2`) for `findUnique`/`update`/`delete`. Multi-field `@relation(fields: [...], references: [...])` for composite FKs. Fully type-safe generated client.

**TypeORM** — Multiple `@PrimaryColumn()` decorators. All operations use object-based where clauses (`{ field1: val1, field2: val2 }`). `@JoinColumn` accepts an array for composite FKs. `save()` does upsert based on all PK fields.

**Sequelize** — Supports composite PK definition but `findByPk()` does not work with composite keys (must use `findOne({ where })`). Composite FK support requires workarounds or raw SQL.

**Drizzle** — `primaryKey({ columns: [col1, col2] })` in the table config callback. `foreignKey({ columns: [...], foreignColumns: [...] })` for composite FKs. No special find-by-PK method; all queries use explicit `where` + `and()`. SQL-first philosophy.

### Java/Kotlin

**Hibernate/JPA** — Two approaches: `@IdClass` (flat fields + separate ID class) and `@EmbeddedId` (nested object). PK class must implement `Serializable`, `equals()`, `hashCode()`. `@JoinColumns` (plural) for composite FKs. `@MapsId` connects relationship fields to embedded ID fields. Full relationship support.

**Exposed (Kotlin)** — `PrimaryKey(col1, col2)` in the table object. Only the DSL (SQL-like) API supports composite keys; the DAO (`EntityClass`) API does not. Relationships require manual joins.

### Go ORMs

**GORM** — Multiple `gorm:"primaryKey"` tags. Composite FKs via `foreignKey:Col1,Col2;references:Col1,Col2`. Zero-value problem: PK column with value `0` is treated as "not set."

**Ent** — No composite PK support by design (graph semantics, every node has a single ID). Unique composite indexes are the workaround.

### Ruby

**ActiveRecord (Rails 7.1+)** — `primary_key: [:col1, :col2]` in migrations, `self.primary_key = [:col1, :col2]` in model. `find([a, b])` for lookups. `query_constraints: [:col1, :col2]` for composite FK associations. Pre-7.1 required the `composite_primary_keys` gem.

### Cross-ORM Summary

| ORM | Composite PK | Composite FK | Find by PK | Relationship Support |
|-----|:-----------:|:------------:|:----------:|:-------------------:|
| Diesel (Rust) | Yes | Yes | Tuple | Full |
| SeaORM (Rust) | Yes | Partial | Tuple | Full |
| SQLAlchemy (Python) | Yes | Yes | Tuple | Full |
| Django (Python) | 5.2+ | Limited | Tuple | Partial |
| Prisma (TS) | Yes | Yes | Generated compound | Full |
| TypeORM (TS) | Yes | Yes | Object | Full |
| Sequelize (JS) | Yes | Partial | Broken | Partial |
| Drizzle (TS) | Yes | Yes | Manual where | Manual |
| Hibernate/JPA | Yes | Yes | ID class | Full |
| GORM (Go) | Yes | Yes | Where clause | Full |
| ActiveRecord (Ruby) | 7.1+ | 7.1+ | Array | Partial |

**Key takeaway:** Mature ORMs (Diesel, SQLAlchemy, Hibernate) treat composite keys as first-class citizens where *all* operations work uniformly. The most common API pattern is tuple-based identity (`find((a, b))`). Composite foreign keys are universally harder than composite PKs — even established ORMs have rougher edges there.

## Common SQL Patterns Requiring Composite Keys

### 1. Junction Tables (Many-to-Many)

The most common use case. The junction table's PK is the combination of FKs to both related tables.

```sql
CREATE TABLE enrollment (
    student_id INTEGER NOT NULL REFERENCES student(id),
    course_id INTEGER NOT NULL REFERENCES course(id),
    enrolled_at TIMESTAMP DEFAULT NOW(),
    grade VARCHAR(2),
    PRIMARY KEY (student_id, course_id)
);
```

Junction tables often accumulate extra attributes (grade, enrolled_at, role) that make them first-class entities requiring full CRUD support, not just a hidden link table.

**Toasty gap:** Many-to-many relationships are listed as a separate roadmap item. Composite key support is a prerequisite — junction tables are inherently composite-keyed.

### 2. Multi-Tenant Data Isolation

Tenant ID appears as the first column in every composite PK, enabling partition-level isolation and efficient tenant-scoped queries.

```sql
CREATE TABLE tenant_document (
    tenant_id UUID NOT NULL REFERENCES tenant(id),
    document_id UUID NOT NULL DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    PRIMARY KEY (tenant_id, document_id)
);

-- All queries are scoped: WHERE tenant_id = $1 AND ...
```

**Why composite PKs:** Enforces isolation at the database level. PK index prefix enables efficient tenant-scoped queries. Maps directly to DynamoDB's partition/local key model.

**Toasty gap:** The `#[key(partition = ..., local = ...)]` syntax already models this. The gaps are in relationship handling when both sides use composite keys.

### 3. Time-Series Data

```sql
CREATE TABLE sensor_reading (
    sensor_id INTEGER NOT NULL,
    recorded_at TIMESTAMP NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (sensor_id, recorded_at)
);
```

**Why composite PKs:** Natural ordering by sensor then time. Range scans on `recorded_at` within a sensor are efficient. Supports table partitioning by time ranges.

### 4. Hierarchical Data (Closure Table)

```sql
CREATE TABLE category_closure (
    ancestor_id INTEGER NOT NULL REFERENCES category(id),
    descendant_id INTEGER NOT NULL REFERENCES category(id),
    depth INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (ancestor_id, descendant_id)
);
```

### 5. Composite Foreign Keys Referencing Composite PKs

A child table references a parent with a composite PK — all parent PK columns appear in the child as FK columns.

```sql
CREATE TABLE order_item (
    order_id INTEGER NOT NULL REFERENCES "order"(id),
    item_number INTEGER NOT NULL,
    PRIMARY KEY (order_id, item_number)
);

CREATE TABLE order_item_shipment (
    id SERIAL PRIMARY KEY,
    order_id INTEGER NOT NULL,
    item_number INTEGER NOT NULL,
    shipment_id INTEGER NOT NULL REFERENCES shipment(id),
    FOREIGN KEY (order_id, item_number)
        REFERENCES order_item(order_id, item_number)
);
```

**Toasty gap:** This is the hardest pattern. The engine simplification and lowering layers assume single-field FKs in multiple places. Fixing this is the core of the composite key work.

### 6. Versioned Records

```sql
CREATE TABLE document_version (
    document_id INTEGER NOT NULL REFERENCES document(id),
    version INTEGER NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    PRIMARY KEY (document_id, version)
);
```

### 7. Composite Unique Constraints vs Composite Primary Keys

Some applications prefer a surrogate PK with a composite unique constraint:

```sql
-- Surrogate PK + composite unique
CREATE TABLE enrollment (
    id SERIAL PRIMARY KEY,
    student_id INTEGER NOT NULL,
    course_id INTEGER NOT NULL,
    UNIQUE (student_id, course_id)
);
```

Trade-offs: surrogate PKs simplify FKs (single column) and URL design, but composite PKs are more storage-efficient and semantically meaningful. ORMs that don't support composite PKs (Django pre-5.2, Tortoise, Ent) force the surrogate pattern.

**Toasty should support both patterns** — composite PKs for direct use and composite unique constraints for the surrogate approach.

## Implementation Plan

### Phase 1: Engine Simplification — Composite PK/FK Handling

Fix the `todo!()` panics in the engine simplification layer so that queries involving composite keys pass through without crashing, even if not fully optimized.

**Files:**
- `engine/simplify/expr_binary_op.rs` — Handle composite PKs and FKs in equality simplification. For composite keys, generate an AND of per-field comparisons.
- `engine/simplify/expr_in_list.rs` — Handle IN-list for composite PKs. Generate `(col1, col2) IN ((v1, v2), (v3, v4))` or equivalent AND/OR tree.
- `engine/simplify/rewrite_root_path_expr.rs` — Rewrite path expressions for composite PKs.

**Approach:** Where a single-field operation currently destructures `let [field] = &fields[..]`, extend to iterate over all fields and combine with AND expressions.

### Phase 2: Subquery Lifting for Composite FKs

Extend the subquery lifting optimization to handle composite foreign keys in BelongsTo and HasOne relationships.

**Files:**
- `engine/simplify/lift_in_subquery.rs` — Remove the `assert_eq!(len, 1)` and handle multi-field FKs. For the optimization path, generate AND of per-field comparisons. For the fallback IN subquery path, generate tuple-based IN expressions or multiple correlated conditions.

**Approach:** The existing single-field logic maps `fk_field.source -> fk_field.target`. For composite keys, do the same for each field pair and combine with AND.

### Phase 3: Engine Lowering — Composite FK Relationships

Fix insert and relationship lowering to handle composite FKs.

**Files:**
- `engine/lower/insert.rs` — When lowering BelongsTo in insert operations, set all FK fields from the related record's PK fields, not just one.
- `engine/lower.rs` — Handle composite FKs in relationship lowering. Generate multi-column join conditions.

### Phase 4: DynamoDB Driver — Batch Operations with Composite Keys

**Files:**
- `driver-dynamodb/op/update_by_key.rs` — Support batch updates with multiple keys (iterate and issue individual UpdateItem calls if needed).
- `driver-dynamodb/op/delete_by_key.rs` — Support batch deletes. Remove the single-key panic.
- `driver-dynamodb/op/create_table.rs` — Support composite unique indexes (Global Secondary Indexes with multiple key columns where DynamoDB allows it).

### Phase 5: Test Coverage

Fill in the stubbed tests and add new ones covering all composite key combinations:

**Existing stubs to implement:**
- `has_many_when_pk_is_composite` — Parent has composite PK, child has single FK pointing to it
- `has_many_when_fk_and_pk_are_composite` — Both sides have composite keys

**New tests to add:**

| Test | Description |
|------|-------------|
| `composite_pk_crud` | Full CRUD (create, read, update, delete) on a model with 2+ key fields |
| `composite_pk_three_fields` | Composite PK with 3 fields to test beyond the 2-field case |
| `composite_fk_belongs_to` | BelongsTo where the FK is composite (references a composite PK) |
| `composite_fk_has_one` | HasOne with composite FK |
| `composite_key_pagination` | Cursor-based pagination with composite PK ordering |
| `composite_key_batch_operations` | Batch get/update/delete with composite keys |
| `composite_key_scoped_queries` | Scoped queries (e.g., `user.todos().filter_by_id(...)`) with composite keys |
| `composite_key_update_non_key_fields` | Update non-key fields on a composite-keyed model |
| `composite_key_unique_constraint` | Composite unique constraint (not PK) behavior |
| `junction_table_pattern` | Many-to-many junction table with composite PK and extra attributes |
| `multi_tenant_pattern` | Tenant-scoped models with `(tenant_id, entity_id)` composite PKs |

### Phase 6: Documentation and Examples

- Update the user guide with composite key patterns and best practices
- Add examples for junction table, multi-tenant, and time-series patterns
- Document the `#[key]` vs `#[key(partition = ..., local = ...)]` distinction and when to use each

## Design Decisions

### Tuple-Based Identity

Following Diesel and SQLAlchemy's lead, composite key identity should be represented as tuples. The current generated methods (`get_by_field1_and_field2(val1, val2)`) are a good API. For batch operations, the tuple-of-references pattern (`filter_by_field1_and_field2_batch([(&a, &b), ...])`) is also solid.

### AND Composition for Multi-Field Conditions

When a single-field operation like `pk_field = value` needs to become a composite operation, the standard approach is:

```
pk_field1 = value1 AND pk_field2 = value2
```

This maps cleanly to SQL `WHERE` clauses and DynamoDB key conditions. The engine's `stmt::ExprAnd` already supports this.

### IN-List with Composite Keys

For batch lookups, composite IN can be expressed as:

```sql
-- Row-value syntax (PostgreSQL, MySQL 8.0+, SQLite)
WHERE (col1, col2) IN ((v1a, v2a), (v1b, v2b))

-- Equivalent OR-of-ANDs (universal)
WHERE (col1 = v1a AND col2 = v2a) OR (col1 = v1b AND col2 = v2b)
```

The OR-of-ANDs form is more portable across databases. The engine should generate this form and let the SQL serializer optimize to row-value syntax where supported.

### Composite FK Optimization

The subquery lifting optimization (`lift_in_subquery.rs`) currently rewrites:

```sql
-- Before: subquery
user_id IN (SELECT id FROM users WHERE name = 'Alice')
-- After: direct comparison
user_id = <alice_id>
```

For composite FKs, the rewrite becomes:

```sql
-- Before: correlated subquery
(order_id, item_number) IN (SELECT order_id, item_number FROM order_items WHERE ...)
-- After: direct comparison
order_id = <val1> AND item_number = <val2>
```

The same optimization logic applies — just iterated over each FK field pair.

## Testing Strategy

- All new tests go in the integration suite (`toasty-driver-integration-suite`) to run against all database backends
- Use the existing `#[driver_test]` macro for multi-database testing
- Use the matrix testing infrastructure (`composite` dimension) where appropriate
- Each phase should have passing tests before moving to the next phase
- No unit tests in source code per project convention
