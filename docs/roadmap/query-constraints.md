# Query Constraints & Filtering

## Overview

This document identifies gaps in Toasty's query constraint support compared to mature ORMs, and outlines potential additions for building web applications.

### Terminology

A "query constraint" refers to any predicate used in the WHERE clause of a query. In Toasty, constraints are built using:

- **Generated filter methods** (`Model::filter_by_<field>()`) for indexed/key fields
- **Generic `.filter()` method** accepting `Expr<bool>` for arbitrary conditions
- **`Model::FIELDS.<field>()` paths** combined with comparison methods (`.eq()`, `.gt()`, etc.)

## Core AST Support Without User API

These expression types exist in `toasty-core` (`crates/toasty-core/src/stmt/expr.rs`) and have SQL serialization, but lack a typed user-facing API on `Path<T>` or `Expr<T>`:

| Expression | Core AST | SQL Serialized | User API | Notes |
|---|---|---|---|---|
| NOT | `ExprNot` | Yes | No `.not()` on `Expr<bool>` | Core + SQL work, but no ergonomic user API |
| IS NULL | `ExprIsNull` | Yes | No `.is_null()` on `Path<T>` | Core + SQL work, no user API |
| LIKE | `ExprPattern::Like` | Yes | None | SQL serialization exists |
| Begins With | `ExprPattern::BeginsWith` | Yes | None | Converted to `LIKE 'prefix%'` in SQL |
| EXISTS | `ExprExists` | Yes | None on user API | Used internally by engine |
| COUNT | `ExprFunc::Count` | Yes | None | Internal use only |

## ORM Comparison

The following table compares Toasty's constraint support against 8 mature ORMs, highlighting missing features:

| Feature | Toasty | Prisma | Drizzle | Django | SQLAlchemy | Diesel | SeaORM | Hibernate |
|---|---|---|---|---|---|---|---|---|---|
| **Logical Operators** | | | | | | | | |
| NOT | AST only | Yes | Yes | Yes | Yes | Per-op | Yes | Yes |
| **Null Handling** | | | | | | | | |
| IS NULL | AST only | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| IS NOT NULL | No | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| **Set Operations** | | | | | | | | |
| NOT IN | No | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| **Range** | | | | | | | | |
| BETWEEN | No | Via gt+lt | Yes | Yes | Yes | Yes | Yes | Yes |
| **String Operations** | | | | | | | | |
| LIKE | AST only | Via contains | Yes | Yes | Yes | Yes | Yes | Yes |
| Contains (substring) | No | Yes | Manual | Yes | Yes | Manual | Yes | Manual |
| Starts with | AST only | Yes | Manual | Yes | Yes | Manual | Yes | Manual |
| Ends with | No | Yes | Manual | Yes | Yes | Manual | Yes | Manual |
| Case-insensitive (ILIKE) | No | Yes | Yes | Yes | Yes | Pg only | No | Manual |
| Regex | No | No | No | Yes | Yes | No | No | No |
| Full-text search | No | Preview | No | Yes (Pg) | Dialect | Crate | No | Extension |
| **Relation Filtering** | | | | | | | | |
| Filter by related fields | No | Yes | Via join | Yes | Yes | Via join | Via join | Via join |
| Has related (some/none/every) | No | Yes | Via exists | Via exists | Yes | Via exists | Via join | Via exists |
| **Aggregation** | | | | | | | | |
| COUNT / SUM / AVG / etc. | No | Limited | Yes | Yes | Yes | Yes | Yes | Yes |
| GROUP BY | No | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| HAVING | No | No | Yes | Yes | Yes | Yes | Yes | Yes |
| **Advanced** | | | | | | | | |
| Field-to-field comparison | No | No | Yes | Yes | Yes | Yes | Yes | Yes |
| Arithmetic in queries | No | No | Yes | Yes | Yes | Yes | Yes | Yes |
| Raw SQL escape hatch | No | Full query | Inline | Multiple | Inline | Inline | Inline | Native query |
| JSON field queries | No | Limited | Via raw | Yes | Yes | Pg | Via raw | No |
| CASE / WHEN | No | No | No | Yes | Yes | No | No | Yes |
| Dynamic/conditional filters | No | Spread undef | Pass undef | Chain | Chain | BoxableExpr | add_option | Build list |

## Potential Future Work

### Features with Existing Internal Support

These features have core AST and SQL serialization but need user-facing APIs:

**NOT Negation**
- Core AST: `ExprNot` exists with SQL serialization
- Needed: `.not()` method on `Expr<bool>`
- File: `crates/toasty/src/stmt/expr.rs`
- Use case: Excluding results (e.g., "status is NOT deleted", negating complex conditions)

**IS NULL / IS NOT NULL**
- Core AST: `ExprIsNull` exists with SQL serialization
- Needed: `.is_null()` and `.is_not_null()` on `Path<Option<T>>`
- File: `crates/toasty/src/stmt/path.rs`
- Use case: Nullable fields filtering (e.g., "deleted_at IS NULL", "email IS NOT NULL")

**String Pattern Matching**
- Core AST: `ExprPattern::BeginsWith` and `ExprPattern::Like` exist with SQL serialization
- Needed:
  - Add `ExprPattern::EndsWith` and `ExprPattern::Contains` to core AST
  - Add `.contains()`, `.starts_with()`, `.ends_with()` on `Path<String>`
  - Add `.like()` for direct pattern matching
  - Handle LIKE special character escaping (`%`, `_`)
- Files: `crates/toasty/src/stmt/path.rs`, `crates/toasty-core/src/stmt/expr.rs`
- Use case: Search functionality (e.g., search users by name fragment)

**NOT IN**
- Current: `IN` exists but no negated form
- Needed: `ExprNotInList` or negate the `InList` expression, plus `.not_in_set()` user API
- Files: `crates/toasty/src/stmt/path.rs`, `crates/toasty-core/src/stmt/expr.rs`
- Use case: Exclusion lists (e.g., "exclude these IDs from results")

### Features Needing New Implementation

**Case-Insensitive String Matching**
- Current: No support at any layer
- Needed: ILIKE support in SQL serialization (PostgreSQL native, LOWER() wrapper for SQLite/MySQL), plus user API
- Design consideration: How to handle cross-database differences (ILIKE is Pg-only, LOWER()+LIKE is universal but slower)
- Reference: Prisma (`mode: 'insensitive'`), Django (`__iexact`, `__icontains`)
- Use case: User-facing search (e.g., email lookup, name search)

**BETWEEN / Range Queries**
- Current: Users must combine `.ge()` and `.le()` manually
- Needed: Syntactic sugar over AND(ge, le), or a dedicated `ExprBetween`
- File: `crates/toasty/src/stmt/path.rs`
- Reference: Drizzle (`between()`), Django (`__range`), Diesel (`.between()`)
- Use case: Date ranges, price ranges, numeric filtering

**Relation/Association Filtering**
- Current: Scoped queries exist but no way to filter a top-level query by related model fields
- Needed: JOIN or EXISTS subquery generation in the engine, plus user API design
- Complexity: High - requires significant engine work
- Reference: Prisma (`some`/`none`/`every`), Django (`__` traversal), SQLAlchemy (`.any()`/`.has()`)
- Use case: Filtering parents by child attributes (e.g., "users who have at least one order over $100")

**Field-to-Field Comparison**
- Current: `Path::eq()` requires `IntoExpr<T>`, which accepts values but should also accept paths
- Needed: Ensure `Path<T>` implements `IntoExpr<T>` and codegen supports cross-field comparisons
- Reference: Django (`F()` expressions), SQLAlchemy (column comparison)
- Use case: Comparing two columns (e.g., "updated_at > created_at", "balance > minimum_balance")

**Arithmetic Operations in Queries**
- Current: No support - `BinaryOp` only includes comparison operators (Eq, Ne, Gt, Ge, Lt, Le)
- Needed:
  - Add arithmetic operators to AST: `Add`, `Subtract`, `Multiply`, `Divide`, `Modulo`
  - SQL serialization for arithmetic expressions (standard across databases)
  - User API to build arithmetic expressions (e.g., `.add()`, `.multiply()`, operator overloading, or expression builder)
  - Type handling for arithmetic results (ensure type safety)
- Files: `crates/toasty-core/src/stmt/op_binary.rs`, `crates/toasty-core/src/stmt/expr.rs`, `crates/toasty/src/stmt/path.rs`
- Reference:
  - Django: `F('price') * F('quantity') > 100`
  - SQLAlchemy: `column('price') * column('quantity') > 100`
  - Diesel: `price.eq(quantity * 2)`
  - Drizzle: `sql`price * quantity > 100``
- Use cases:
  - Computed comparisons: `WHERE age <= 2 * years_in_school`
  - Price calculations: `WHERE price * quantity > 1000`
  - Time differences: `WHERE (end_time - start_time) > 3600`
  - Percentage calculations: `WHERE (actual / budget) * 100 > 110`
  - Complex business rules: `WHERE (base_price * (1 - discount_rate)) > minimum_price`
- Design considerations:
  - Should arithmetic create new expression types or extend `BinaryOp`?
  - How to handle type coercion (int vs float, time arithmetic)?
  - Support for parentheses and operator precedence
  - Whether to support on SELECT side (computed columns) or just WHERE clauses initially

**Aggregate Queries**
- Current: `ExprFunc::Count` exists internally but is not user-facing
- Needed: User-facing API, return type handling, integration with GROUP BY
- Complexity: High - requires significant API design
- Reference: Django's annotation system, SQLAlchemy's `func`
- Use case: Dashboards, analytics, summary views, pagination metadata

**GROUP BY / HAVING**
- Current: No support at any layer
- Needed: AST additions, SQL generation, engine support, user API
- Complexity: High
- Use case: Aggregate queries, reports, analytics, dashboards

**Raw SQL Escape Hatch**
- Current: No support
- Needed: Safe API for parameterized raw SQL fragments within typed queries
- Design consideration: Full raw queries vs. raw fragments within typed queries vs. both
- Reference: Drizzle (`` sql`...` `` templates), SQLAlchemy (`text()`), Diesel (`sql()`)
- Use case: Edge cases that the ORM can't express

**Dynamic / Conditional Query Building**
- Current: Users can chain `.filter()` calls, but no ergonomic way to skip filters when parameters are `None`
- Needed: Pattern for optional filters
- Reference: SeaORM (`Condition::add_option()`), Prisma (spread undefined), Diesel (`BoxableExpression`)
- Use case: Search forms, filter UIs, API endpoints with optional parameters

**Full-Text Search**
- Current: No support
- Complexity: High - database-specific implementations (PostgreSQL tsvector, MySQL FULLTEXT, SQLite FTS5)
- Design consideration: May be best as database-specific extensions rather than a unified API
- Use case: Content-heavy applications (blogs, e-commerce, documentation sites)

**JSON Field Queries**
- Current: No support
- Complexity: High - needs path traversal syntax, type handling, database-specific operators
- Dependency: Depends on JSON/JSONB data type support
- Reference: Django (`field__key__subkey`), SQLAlchemy (`column['key']`)
- Use case: Flexible/schemaless data within relational databases

### Advanced / Niche Features

**Regex Matching**
- Use case: Power-user filtering, data validation queries
- Reference: Django (`__regex`, `__iregex`), SQLAlchemy (`regexp_match()`)

**Array/Collection Operations**
- Use case: PostgreSQL array columns, MongoDB array fields
- Dependency: Requires array type support first
- Reference: Prisma (`has`, `hasEvery`, `hasSome`), Django (ArrayField lookups)

**CASE/WHEN Expressions**
- Use case: Conditional logic within queries for complex business rules
- Reference: Django (`When()`/`Case()`), SQLAlchemy (`case()`)

**Subquery Comparisons (ALL/ANY/SOME)**
- Use case: Advanced filtering like "price > ALL(SELECT price FROM competitors)"
- Reference: Hibernate, SQLAlchemy (`all_()`, `any_()`)

**IS DISTINCT FROM**
- Use case: NULL-safe comparisons without special-casing IS NULL
- Reference: SQLAlchemy (only ORM with native support)

## Implementation Considerations

### Recommended Approach

Based on the analysis above, the following groupings maximize user value:

**Group 1: Expose Existing Internals**
Items with core AST and SQL serialization that only need user-facing methods:
- `.not()` on `Expr<bool>`
- `.is_null()` / `.is_not_null()` on `Path<Option<T>>`
- `.not_in_set()` on `Path<T>` (negate existing `InList`)

Estimated scope: ~100 lines of user-facing API code + integration tests (for remaining items)

**Group 2: String Operations**
Partial AST support that needs completion and exposure:
- Add `ExprPattern::EndsWith` and `ExprPattern::Contains` to core AST
- Add SQL serialization for new pattern variants
- Add `.contains()`, `.starts_with()`, `.ends_with()` to `Path<String>`
- Handle LIKE special character escaping

Estimated scope: ~200 lines across core + SQL + user API

**Group 3: Ergonomic Improvements**
- Case-insensitive matching (ILIKE / LOWER() wrapper)
- `.between()` convenience method
- `.like()` direct exposure
- Conditional/optional filter building helpers

**Group 4: Structural Features**
Requires deeper engine work:
- Relation filtering (JOIN/EXISTS generation)
- Aggregate functions (user-facing COUNT/SUM/etc.)
- GROUP BY / HAVING
- Raw SQL escape hatch

## Reference Implementation Goals

A comprehensive query constraint system would allow users to:

1. Filter on any combination of field conditions using AND, OR, and NOT
2. Check for NULL/non-NULL values
3. Search strings by substring, prefix, and suffix (case-sensitive and case-insensitive)
4. Use IN/NOT IN with both literal lists and subqueries
5. Filter by related model attributes
6. Use at least basic aggregate queries (COUNT)
7. Fall back to raw SQL for anything the ORM can't express

This would put Toasty on par with the filtering capabilities of Diesel and SeaORM, and cover the vast majority of queries needed by typical web applications.
