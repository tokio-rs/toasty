# Toasty ORM - Development Roadmap

This roadmap outlines potential enhancements and missing features for the Toasty ORM.

## Overview

Toasty is an easy-to-use ORM for Rust that supports both SQL and NoSQL databases. This roadmap documents potential future work and feature gaps.

> **User Documentation:** See the [`guide/`](../guide/) directory for complete API documentation and usage examples.

## Feature Areas

### Composite Keys

**[Composite Key Support](./composite-keys.md)** (partial implementation)
- Composite foreign key optimization in query simplification
- Composite PK handling in expression rewriting and IN-list operations
- HasMany/BelongsTo relationships with composite foreign keys referencing composite primary keys
- Junction table / many-to-many patterns with composite keys
- DynamoDB driver: batch delete/update with composite keys, composite unique indexes
- Comprehensive test coverage for all composite key combinations

### Query Capabilities

**[Query Ordering, Limits & Pagination](./order_limit_pagination.md)** | [User Guide](../guide/pagination.md)
- Multi-column ordering convenience method (`.then_by()`)
- Direct `.limit()` method for non-paginated queries
- `.last()` convenience method

**[Query Constraints & Filtering](./query-constraints.md)**
- IS NULL (core AST exists, needs user API)
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

**Partial Model Loading**
- Allow models to have fields that are not loaded by default (e.g. a large `body` column on an `Article` model)
- Fields opt-in via a `#[deferred]` attribute and must be wrapped in a `Deferred<T>` type
- By default, queries skip deferred fields; callers opt-in with `.include(Article::body)` (same API as relation preloading)
- Accessing a `Deferred<T>` that was not loaded either returns an error or panics with a clear message
- Works with primitive types, embedded structs, and embedded enums — just a subset of columns in the same table
  ```rust
  #[toasty::model]
  struct Article {
      #[key]
      id: Id<Self>,
      title: String,
      author: BelongsTo<User>,
      #[deferred]
      body: Deferred<String>,   // not loaded unless explicitly included
  }

  // Load metadata only (no body column fetched)
  let articles = Article::all().collect(&db).await?;

  // Load with body
  let articles = Article::all().include(Article::body).collect(&db).await?;
  ```

**Relationships**
- Many-to-many relationships
- Polymorphic associations
- Nested preloading (multi-level `.include()` support)

### Query Building

**Query Features**
- Subquery improvements
- Better conditional/dynamic query building ergonomics

**Database Function Expressions**
- Allow database-side functions (e.g. `NOW()`, `CURRENT_TIMESTAMP`) as expressions in create and update operations
- User API: field setters accept `toasty::stmt` helpers like `toasty::stmt::now()` that resolve to `core::stmt::ExprFunc` variants
  ```rust
  // Set updated_at to the database's current time instead of a Rust-side value
  user.update()
      .updated_at(toasty::stmt::now())
      .exec(&db)
      .await?;

  // Also usable in create operations
  User::create()
      .name("Alice")
      .created_at(toasty::stmt::now())
      .exec(&db)
      .await?;
  ```
- Extend `ExprFunc` enum in `toasty-core` with new function variants (e.g. `Now`)
- SQL serialization for each function across supported databases (`NOW()` for PostgreSQL/MySQL, `datetime('now')` for SQLite)
- Codegen: update field setter generation to accept both value types and function expressions
- Future: support additional scalar functions (e.g. `COALESCE`, `LOWER`, `UPPER`, `LENGTH`)

**Raw SQL Support**
- Execute arbitrary SQL statements directly
- Parameterized queries with type-safe bindings
- Raw SQL fragments within typed queries (escape hatch for complex expressions)

### Data Modification

**Mutation Result Information**
- Return affected row counts from update operations (how many records were updated)
- Return affected row counts from delete operations (how many records were deleted)
- Better result types that provide operation metadata
- Distinguish between "no rows matched" vs "rows matched but no changes needed"

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

### Toasty Runtime Improvements

**Concurrent Task Execution**
- Replace the current ad-hoc background task with a proper in-flight task manager
- Execute independent parts of an execution plan concurrently
- Track and coordinate multiple in-flight tasks within a single query execution

**Cancellation & Cleanup**
- Detect when the caller drops the future representing query completion
- Perform clean cancellation on drop (rollback any incomplete transactions)
- Ensure no resource leaks or orphaned database state on cancellation

**Internal Instrumentation & Metrics**
- Instrument time spent in each execution phase (planning, simplification, execution, serialization)
- Track CPU time consumed by query planning to detect expensive plans
- Provide internal metrics for diagnosing performance bottlenecks

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
