# Toasty ORM - Development Roadmap

This roadmap outlines potential enhancements and missing features for the Toasty ORM.

## Overview

Toasty is an easy-to-use ORM for Rust that supports both SQL and NoSQL databases. This roadmap documents potential future work and feature gaps.

> **User Documentation:** See the [`guide/`](../guide/) directory for complete API documentation and usage examples.

## Feature Areas

### Query Capabilities

**[Query Ordering, Limits & Pagination](./order_limit_pagination.md)** | [User Guide](../guide/pagination.md)
- Multi-column ordering convenience method (`.then_by()`)
- Direct `.limit()` method for non-paginated queries
- `.last()` convenience method

**[Query Constraints & Filtering](./query-constraints.md)**
- NOT, IS NULL (core AST exists, needs user API)
- String operations: contains, starts with, ends with, LIKE (partial AST support)
- NOT IN
- Case-insensitive matching
- BETWEEN / range queries
- Relation filtering (filter by associated model fields)
- Field-to-field comparison
- Arithmetic operations in queries (add, subtract, multiply, divide, modulo)
- Aggregate queries and GROUP BY / HAVING

### Data Types

**Extended Data Types**
- [Embedded struct & enum support](../design/enums-and-embedded-structs.md) (partial implementation)
- Serde-serialized types (JSON/JSONB columns for arbitrary Rust types)
- Embedded collections (arrays, maps, sets, etc.)

### Relationships & Loading

**Relationships**
- Many-to-many relationships
- Polymorphic associations
- Nested preloading (multi-level `.include()` support)

### Query Building

**Query Features**
- Subquery improvements
- Better conditional/dynamic query building ergonomics

**Raw SQL Support**
- Execute arbitrary SQL statements directly
- Parameterized queries with type-safe bindings
- Raw SQL fragments within typed queries (escape hatch for complex expressions)

### Transactions

**Atomic Batch Operations**
- Cross-database atomic batch API
- Supported across SQL and NoSQL databases
- Type-safe operation batching
- All-or-nothing semantics

**SQL Transaction API**
- Manual transaction control for SQL databases
- BEGIN/COMMIT/ROLLBACK support
- Savepoints and nested transactions
- Isolation level configuration

### Schema Management

**Migrations**
- Schema migration system
- Migration generation
- Rollback support
- Schema versioning
- CLI tools for schema management

### Performance

**Optimization Features**
- Bulk inserts/updates
- Query caching
- Connection pooling improvements

### Developer Experience

**Ergonomic Macros**
- `toasty::query!()` - Succinct query syntax that translates to builder DSL
  ```rust
  // Instead of: User::all().filter(...).order_by(...).collect(&db).await
  toasty::query!(User, filter: ..., order_by: ...).collect(&db).await
  ```
- `toasty::create!()` - Concise record creation syntax
  ```rust
  // Instead of: User::create().name("Alice").age(30).exec(&db).await
  toasty::create!(User, name: "Alice", age: 30).exec(&db).await
  ```
- `toasty::update!()` - Simplified update syntax
  ```rust
  // Instead of: user.update().name("Bob").age(31).exec(&db).await
  toasty::update!(user, name: "Bob", age: 31).exec(&db).await
  ```

**Tooling & Debugging**
- Query logging

## Notes

The roadmap documents describe potential enhancements and missing features. For information about what's currently implemented, refer to the user guide or test the API directly.
