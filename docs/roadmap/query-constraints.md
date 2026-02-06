# Query Constraints & Filtering

## Current State Analysis

This document inventories the query constraint patterns currently supported by Toasty, identifies gaps compared to mature ORMs, and prioritizes missing features for building real web applications.

### Terminology

A "query constraint" refers to any predicate used in the WHERE clause of a query. In Toasty, constraints are built using:

- **Generated filter methods** (`Model::filter_by_<field>()`) for indexed/key fields
- **Generic `.filter()` method** accepting `Expr<bool>` for arbitrary conditions
- **`Model::FIELDS.<field>()` paths** combined with comparison methods (`.eq()`, `.gt()`, etc.)

### What's Implemented & Tested

These constraint patterns have integration tests in [`toasty-driver-integration-suite`](../../crates/toasty-driver-integration-suite/src/tests/) and work end-to-end across all database drivers:

| Constraint | User API | Test Coverage | Example |
|---|---|---|---|
| Equality | `Path::eq()` | Extensive | `User::FIELDS.name().eq("Alice")` |
| Not Equal | `Path::ne()` | Good | `Event::FIELDS.timestamp().ne(10)` |
| Greater Than | `Path::gt()` | Good | `Event::FIELDS.timestamp().gt(10)` |
| Greater or Equal | `Path::ge()` | Good | `Event::FIELDS.timestamp().ge(10)` |
| Less Than | `Path::lt()` | Good | `Event::FIELDS.timestamp().lt(10)` |
| Less or Equal | `Path::le()` | Good | `Event::FIELDS.timestamp().le(10)` |
| AND | `Expr::and()` | Extensive | `expr_a.and(expr_b)` |
| IN (list) | `Path::in_set()` | API exists, not integration-tested | `User::FIELDS.id().in_set(ids)` |
| IN (subquery) | `Path::in_query()` | API exists, not integration-tested | `User::FIELDS.id().in_query(subquery)` |
| Filter by PK | `Model::filter_by_id()` | Extensive | `User::filter_by_id(id)` |
| Filter by index | `Model::filter_by_<field>()` | Good | `User::filter_by_name("Alice")` |
| Composite key query | Partition + local key | Good | `Team::FIELDS.league().eq("MLS").and(Team::FIELDS.name().eq("Portland"))` |

Key test files:
- `one_model_query.rs` - Comparison operators, indexed filters, composite keys
- `has_many_scoped_query.rs` - Constraints on association queries
- `one_model_sort_limit.rs` - ORDER BY (related to constraints)
- `has_many_crud_multi_relations.rs` - Filters with foreign key references

### What's in the AST but Not User-Facing

These expression types exist in `toasty-core` (`crates/toasty-core/src/stmt/expr.rs`) and have SQL serialization, but lack a typed user-facing API on `Path<T>` or `Expr<T>`:

| Expression | Core AST | SQL Serialized | User API | Notes |
|---|---|---|---|---|
| OR | `ExprOr` | Yes | **No `.or()` on `Expr<bool>`** | Core + SQL work, but no ergonomic user API |
| NOT | `ExprNot` | Yes | **No `.not()` on `Expr<bool>`** | Same situation |
| IS NULL | `ExprIsNull` | Yes | **No `.is_null()` on `Path<T>`** | Core + SQL work, no user API |
| LIKE | `ExprPattern::Like` | Yes | **None** | SQL serialization exists |
| Begins With | `ExprPattern::BeginsWith` | Yes | **None** | Converted to `LIKE 'prefix%'` in SQL |
| EXISTS | `ExprExists` | Yes | **None on user API** | Used internally by engine |
| COUNT | `ExprFunc::Count` | Yes | **None** | Internal use only |

### What's Missing Entirely

These features have no implementation at any layer:

- BETWEEN / range queries
- Case-insensitive comparisons (ILIKE)
- String contains / ends_with
- NOT IN
- IS NOT NULL (separate from IS NULL negation)
- Regex matching
- Field-to-field comparison
- HAVING clauses
- Aggregate filtering
- JSON field queries
- Full-text search
- Dynamic/conditional query building ergonomics

---

## ORM Comparison

The following table compares Toasty's constraint support against 8 mature ORMs. Features marked as "AST only" exist in Toasty's internal representation but are not exposed to users.

| Feature | Toasty | Prisma | Drizzle | Django | SQLAlchemy | Diesel | SeaORM | Hibernate |
|---|---|---|---|---|---|---|---|---|
| **Basic Comparisons** | | | | | | | | |
| eq / ne / gt / ge / lt / le | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| **Logical Operators** | | | | | | | | |
| AND | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| OR | AST only | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| NOT | AST only | Yes | Yes | Yes | Yes | Per-op | Yes | Yes |
| **Null Handling** | | | | | | | | |
| IS NULL | AST only | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| IS NOT NULL | No | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| **Set Operations** | | | | | | | | |
| IN (list) | API exists | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| IN (subquery) | API exists | No | Yes | Yes | Yes | Yes | Yes | Yes |
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
| Raw SQL escape hatch | No | Full query | Inline | Multiple | Inline | Inline | Inline | Native query |
| JSON field queries | No | Limited | Via raw | Yes | Yes | Pg | Via raw | No |
| CASE / WHEN | No | No | No | Yes | Yes | No | No | Yes |
| Dynamic/conditional filters | No | Spread undef | Pass undef | Chain | Chain | BoxableExpr | add_option | Build list |

---

## Priority Roadmap

Features are prioritized by how frequently they arise when building real web applications, weighted by:
- How many applications need the feature (breadth)
- Whether the lack of the feature forces users to drop to raw SQL (severity)
- How much internal infrastructure already exists (effort)

### P0: Essential - Required for basic CRUD applications

These are blocking for any non-trivial application. Most have partial internal support already.

#### 1. OR Conditions
- **Why:** Nearly every search/filter UI needs OR logic (e.g., "status is active OR pending")
- **Current state:** `ExprOr` exists in core AST, SQL serialization works, but `Expr<bool>` has no `.or()` method
- **Effort:** Low - just needs user-facing API on `Expr<bool>` (similar to existing `.and()`)
- **Reference:** Every surveyed ORM supports this
- **Files:** `crates/toasty/src/stmt/expr.rs` (add `.or()`)

#### 2. NOT Negation
- **Why:** Excluding results is fundamental (e.g., "status is NOT deleted", negating complex conditions)
- **Current state:** `ExprNot` exists in core AST with SQL serialization, no user API
- **Effort:** Low - needs `.not()` method on `Expr<bool>`
- **Reference:** Every surveyed ORM supports this
- **Files:** `crates/toasty/src/stmt/expr.rs` (add `.not()`)

#### 3. IS NULL / IS NOT NULL
- **Why:** Nullable fields are ubiquitous; filtering on presence/absence is basic (e.g., "deleted_at IS NULL", "email IS NOT NULL")
- **Current state:** `ExprIsNull` exists in core with SQL serialization, no user API
- **Effort:** Low - needs `.is_null()` and `.is_not_null()` on `Path<Option<T>>`
- **Reference:** Every surveyed ORM supports this
- **Files:** `crates/toasty/src/stmt/path.rs`

#### 4. String Contains / Starts With / Ends With
- **Why:** Search functionality is one of the first features any web app needs (e.g., search users by name fragment)
- **Current state:** `ExprPattern::BeginsWith` and `ExprPattern::Like` exist in core with SQL serialization, no user API
- **Effort:** Medium - needs `.contains()`, `.starts_with()`, `.ends_with()` on `Path<String>`, plus LIKE pattern escaping
- **Reference:** Prisma (`contains`, `startsWith`, `endsWith`), Django (`__contains`, `__startswith`, `__endswith`), SeaORM (`.contains()`, `.starts_with()`, `.ends_with()`)
- **Files:** `crates/toasty/src/stmt/path.rs`, potentially `crates/toasty-core/src/stmt/expr.rs` (for `EndsWith` pattern variant)

#### 5. NOT IN
- **Why:** Exclusion lists are common (e.g., "exclude these IDs from results")
- **Current state:** `IN` exists but no negated form
- **Effort:** Low - needs `ExprNotInList` or negate the `InList` expression, plus `.not_in_set()` user API
- **Reference:** Every surveyed ORM supports this
- **Files:** `crates/toasty/src/stmt/path.rs`, `crates/toasty-core/src/stmt/expr.rs`

### P1: Important - Required for typical web applications

These come up frequently in real applications but have workarounds or are needed for specific feature sets.

#### 6. Case-Insensitive String Matching
- **Why:** User-facing search is almost always case-insensitive (e.g., email lookup, name search)
- **Current state:** No support at any layer
- **Effort:** Medium - needs ILIKE support in SQL serialization (PostgreSQL native, LOWER() wrapper for SQLite/MySQL), plus user API (e.g., `.eq_ignore_case()`, `.contains_ignore_case()` or a `mode` parameter)
- **Reference:** Prisma (`mode: 'insensitive'`), Django (`__iexact`, `__icontains`), Diesel (`.ilike()` Pg-only), Drizzle (`ilike()`)
- **Design consideration:** How to handle cross-database differences (ILIKE is Pg-only, LOWER()+LIKE is universal but slower)

#### 7. BETWEEN / Range Queries
- **Why:** Date ranges, price ranges, and numeric filtering are common in dashboards and listing pages
- **Current state:** No support; users must combine `.ge()` and `.le()` manually
- **Effort:** Low - syntactic sugar over AND(ge, le), or a dedicated `ExprBetween`
- **Reference:** Drizzle (`between()`), Django (`__range`), Diesel (`.between()`), SeaORM (`.between()`)
- **Files:** `crates/toasty/src/stmt/path.rs`

#### 8. Relation/Association Filtering
- **Why:** Filtering parents by child attributes is extremely common (e.g., "users who have at least one order over $100")
- **Current state:** Scoped queries exist (e.g., `user.todos().query(...)`) but no way to filter a top-level query by related model fields
- **Effort:** High - requires JOIN or EXISTS subquery generation in the engine, plus user API design
- **Reference:** Prisma (`some`/`none`/`every`), Django (`__` traversal), SQLAlchemy (`.any()`/`.has()`), ActiveRecord (`.joins().where()`)
- **Design consideration:** Prisma's `some`/`every`/`none` quantifiers are the gold standard for ergonomics; Django's double-underscore traversal is the most concise

#### 9. Field-to-Field Comparison
- **Why:** Comparing two columns is needed for business logic (e.g., "updated_at > created_at", "balance > minimum_balance")
- **Current state:** The `Path::eq()` etc. methods require `IntoExpr<T>`, which accepts values but paths of other fields should also work
- **Effort:** Medium - needs `Path<T>` to implement `IntoExpr<T>` (it already does), but codegen may need adjustment to make cross-field comparisons ergonomic
- **Reference:** Django (`F()` expressions), SQLAlchemy (column comparison), Diesel (column-to-column)

#### 10. Aggregate Queries (COUNT, SUM, AVG, MIN, MAX)
- **Why:** Dashboards, analytics, and summary views need aggregates; COUNT is needed even for pagination metadata
- **Current state:** `ExprFunc::Count` exists internally but is not user-facing
- **Effort:** High - needs user-facing API, return type handling, and integration with GROUP BY
- **Reference:** Every mature ORM supports this; Django's annotation system and SQLAlchemy's `func` are the most flexible

#### 11. GROUP BY / HAVING
- **Why:** Aggregate queries are incomplete without grouping; used for reports, analytics, dashboards
- **Current state:** No support at any layer
- **Effort:** High - needs AST additions, SQL generation, engine support, and user API
- **Reference:** Every SQL ORM supports this (Prisma is the notable exception for HAVING)

### P2: Valuable - Needed for specific application types

These are important for certain classes of applications but not universally needed.

#### 12. Raw SQL Escape Hatch
- **Why:** Every ORM needs an escape hatch for queries it can't express; prevents users from abandoning the ORM entirely for edge cases
- **Current state:** No support
- **Effort:** Medium - needs a safe API for parameterized raw SQL fragments within otherwise typed queries
- **Reference:** Drizzle (`` sql`...` `` templates), SQLAlchemy (`text()`), ActiveRecord (string conditions), Diesel (`sql()`)
- **Design consideration:** Full raw queries vs. raw fragments within typed queries vs. both

#### 13. Dynamic / Conditional Query Building
- **Why:** Search forms, filter UIs, and API endpoints with optional parameters need to conditionally add constraints
- **Current state:** Users can chain `.filter()` calls, but there's no ergonomic way to skip a filter when a parameter is `None`
- **Effort:** Low-Medium - could follow SeaORM's `add_option()` pattern or Prisma's "skip undefined" approach
- **Reference:** SeaORM (`Condition::add_option()`), Prisma (spread undefined), Diesel (`BoxableExpression`)

#### 14. LIKE Pattern Matching (Direct)
- **Why:** Useful for structured pattern matching beyond simple contains/startsWith/endsWith
- **Current state:** `ExprPattern::Like` exists in core with SQL serialization, no user API
- **Effort:** Low - expose existing implementation
- **Files:** `crates/toasty/src/stmt/path.rs`

#### 15. Full-Text Search
- **Why:** Critical for content-heavy applications (blogs, e-commerce, documentation sites)
- **Current state:** No support
- **Effort:** High - database-specific implementations (PostgreSQL tsvector, MySQL FULLTEXT, SQLite FTS5), hard to abstract uniformly
- **Reference:** Django (comprehensive Pg support), Prisma (preview), Diesel (via crate)
- **Design consideration:** May be best as database-specific extensions rather than a unified API

#### 16. JSON Field Queries
- **Why:** JSON columns are increasingly common for flexible/schemaless data within relational databases
- **Current state:** No support
- **Effort:** High - needs path traversal syntax, type handling, database-specific operators
- **Reference:** Django (`field__key__subkey`), SQLAlchemy (`column['key']`), Diesel (Pg JSONB methods)
- **Design consideration:** Depends on JSON/JSONB data type support (tracked under "Extended Data Types" in the main roadmap)

### P3: Future - Niche or advanced use cases

#### 17. Regex Matching
- **Why:** Power-user filtering, data validation queries
- **Reference:** Django (`__regex`, `__iregex`), SQLAlchemy (`regexp_match()`)

#### 18. Array/Collection Operations
- **Why:** PostgreSQL array columns, MongoDB array fields
- **Reference:** Prisma (`has`, `hasEvery`, `hasSome`), Django (ArrayField lookups), Diesel (Pg array methods)
- **Dependency:** Requires array type support first

#### 19. CASE/WHEN Expressions in Filters
- **Why:** Conditional logic within queries for complex business rules
- **Reference:** Django (`When()`/`Case()`), SQLAlchemy (`case()`), Hibernate (`selectCase()`)

#### 20. Subquery Comparisons (ALL/ANY/SOME Quantifiers)
- **Why:** Advanced filtering like "price > ALL(SELECT price FROM competitors)"
- **Reference:** Hibernate (full support), SQLAlchemy (`all_()`, `any_()`), Diesel (Pg `any()`)

#### 21. IS DISTINCT FROM / NULL-Safe Equality
- **Why:** NULL-safe comparisons without special-casing IS NULL
- **Reference:** SQLAlchemy (only ORM with native support)

---

## Implementation Strategy

Based on the analysis above, the recommended implementation order follows the principle of maximizing user-facing value per unit of effort:

### Wave 1: Low-hanging fruit (expose existing internals)

Items 1-3 and 5 already have core AST and SQL serialization. They only need user-facing methods:

1. Add `.or()` to `Expr<bool>` (mirrors existing `.and()`)
2. Add `.not()` to `Expr<bool>`
3. Add `.is_null()` / `.is_not_null()` to `Path<Option<T>>`
4. Add `.not_in_set()` to `Path<T>` (negate existing `InList`)

**Estimated scope:** ~100 lines of user-facing API code + integration tests

### Wave 2: String operations

Item 4 has partial AST support (`BeginsWith`, `Like`) that needs to be completed and exposed:

1. Add `ExprPattern::EndsWith` and `ExprPattern::Contains` to core AST
2. Add SQL serialization for new pattern variants
3. Add `.contains()`, `.starts_with()`, `.ends_with()` to `Path<String>`
4. Handle LIKE special character escaping (`%`, `_`)

**Estimated scope:** ~200 lines across core + SQL + user API

### Wave 3: Ergonomic improvements

Items 6-7 and 13-14 improve daily developer experience:

1. Case-insensitive matching (ILIKE / LOWER() wrapper)
2. `.between()` convenience method
3. `.like()` direct exposure
4. Conditional/optional filter building helpers

### Wave 4: Structural features

Items 8-11 require deeper engine work:

1. Relation filtering (JOIN/EXISTS generation)
2. Aggregate functions (user-facing COUNT/SUM/etc.)
3. GROUP BY / HAVING
4. Raw SQL escape hatch

---

## Success Criteria

A complete query constraint system for MVP should allow users to:

1. Filter on any combination of field conditions using AND, OR, and NOT
2. Check for NULL/non-NULL values
3. Search strings by substring, prefix, and suffix (case-sensitive and case-insensitive)
4. Use IN/NOT IN with both literal lists and subqueries
5. Filter by related model attributes
6. Use at least basic aggregate queries (COUNT)
7. Fall back to raw SQL for anything the ORM can't express

This would put Toasty on par with the filtering capabilities of Diesel and SeaORM, and cover the vast majority of queries needed by typical web applications.
