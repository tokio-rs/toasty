# Toasty Architecture Overview

## Project Structure

Toasty is an ORM for Rust that supports SQL and NoSQL databases. The codebase is a Cargo workspace with separate crates for each layer.

## Design Principles

### Exposing database operations

Toasty does not hide differences between the databases it targets. When a query
method maps onto an operator the database already has, Toasty passes it through
and keeps the database's behavior instead of normalizing it to match the other
backends.

This produces two rules for query methods:

- A pass-through method keeps each backend's behavior, and the documentation
  states what each backend does. `.like()` lowers to the database's own `LIKE`,
  whose case sensitivity differs between SQLite, PostgreSQL, and MySQL.
- A method that maps to a backend-specific operator is offered only on backends
  that have that operator. `.ilike()` maps to PostgreSQL's `ILIKE`, which no
  other target has, so it is rejected elsewhere (`engine/verify.rs`, gated on
  `Capability::native_ilike`) rather than emulated.

Toasty implements an operation on every backend only when it can give the
operation identical semantics on all of them. `.starts_with()` is meant to be
that: a case-sensitive prefix match has one meaning that every backend can
express (DynamoDB's `begins_with`, a prefix match on SQL), so it is offered
everywhere. A uniform `.ilike()` would not qualify, because the backends
disagree on how to fold case. (The SQL lowering of `.starts_with()` is not yet
case-sensitive on SQLite and MySQL; see issue #936.)

## Crates

### 1. toasty
User-facing crate with query engine and runtime.

**Key Components**:
- `engine/`: Multi-phase query compilation and execution pipeline
  - See [Query Engine Architecture](./query-engine.md) for detailed documentation
- `stmt/`: Typed statement builders (wrappers around `toasty_core::stmt` types)
- `relation/`: Relationship abstractions (HasMany, BelongsTo, HasOne)
- `model.rs`: Model trait and ID generation

**Query Execution Pipeline** (high-level):
```
Statement AST → Simplify → Lower → Plan → Execute → Results
```

The engine compiles queries into a mini-program of actions executed by an interpreter. For details on HIR, MIR, and the full compilation pipeline, see [Query Engine Architecture](./query-engine.md).

### 2. toasty-core
Shared types used by all other crates: schema representations, statement AST, and driver interface.

**Key Components**:
- `schema/`: Model and database schema definitions
  - `app/`: Model-level definitions (fields, relations, constraints)
  - `db/`: Database-level table and column definitions
  - `mapping/`: Maps between models and database tables
  - `builder/`: Schema construction utilities
  - `verify/`: Schema validation
- `stmt/`: Statement AST nodes for queries, inserts, updates, deletes
- `driver/`: Driver interface, capabilities, and operations

### 3. toasty-macros (code generation)
The `toasty-macros` crate contains both the proc-macro entry points and the code generation logic. It generates Rust code from the `#[derive(Model)]` and `#[derive(Embed)]` macros.

**Key Components**:
- `schema/`: Parses model attributes into schema representation
- `expand/`: Generates implementations for models
  - `model.rs`: Model trait implementation
  - `query.rs`: Query builder methods
  - `create.rs`: Create/insert builders
  - `update.rs`: Update builders
  - `relation.rs`: Relationship methods
  - `fields.rs`: Field accessors
  - `filters.rs`: Filter method generation
  - `schema.rs`: Runtime schema generation

### 4. toasty-driver-*
Database-specific driver implementations.

**Supported Databases**:
- `toasty-driver-sqlite`: SQLite implementation
- `toasty-driver-postgresql`: PostgreSQL implementation  
- `toasty-driver-mysql`: MySQL implementation
- `toasty-driver-dynamodb`: DynamoDB implementation

### 5. toasty-sql
Converts statement AST to SQL strings. Used by SQL-based drivers.

**Key Components**:
- `serializer/`: SQL generation with dialect support
  - `flavor.rs`: Database-specific SQL dialects
  - `statement.rs`: Statement serialization
  - `expr.rs`: Expression serialization
  - `ty.rs`: Type serialization
- `stmt/`: SQL-specific statement types

## Further Reading

- [Query Engine Architecture](./query-engine.md) - Query compilation and execution pipeline
- [Type System](./type-system.md) - Type system design and conversions
- [Path System](./path-system.md) - Field references, the typed/untyped layers, and variant paths
