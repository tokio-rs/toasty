# Toasty Architecture Overview

## Project Structure

Toasty is an ORM for Rust that supports SQL and NoSQL databases. The codebase is a Cargo workspace with separate crates for each layer.

## Crates

### 1. toasty
User-facing crate with query engine and runtime.

**Key Components**:
- `engine/`: Multi-phase query compilation and execution pipeline
  - See [Query Engine Architecture](architecture/query-engine.md) for detailed documentation
- `stmt/`: Typed statement builders (wrappers around `toasty_core::stmt` types)
- `relation/`: Relationship abstractions (HasMany, BelongsTo, HasOne)
- `model.rs`: Model trait and ID generation

**Query Execution Pipeline** (high-level):
```
Statement AST → Simplify → Lower → Plan → Execute → Results
```

The engine compiles queries into a mini-program of actions executed by an interpreter. For details on HIR, MIR, and the full compilation pipeline, see [Query Engine Architecture](architecture/query-engine.md).

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

### 3. toasty-codegen
Generates Rust code from the `#[derive(Model)]` macro.

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

- [Query Engine Architecture](architecture/query-engine.md) - Query compilation and execution pipeline
- [Change Patterns](architecture/change-patterns.md) - Where to make different types of changes
