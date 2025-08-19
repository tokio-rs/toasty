# Toasty Architecture Overview

## Project Structure

Toasty is a multi-database ORM for Rust that follows a layered architecture with clear separation of concerns. The codebase is organized as a Cargo workspace with distinct crates for each architectural layer.

## Core Architecture Layers

### 1. toasty-core (Foundation Layer)
**Purpose**: Defines core abstractions, schema representations, and statement AST.

**Key Components**:
- `schema/`: Application and database schema definitions
  - `app/`: Application-level model definitions (fields, relations, constraints)
  - `db/`: Database-level table and column definitions
  - `mapping/`: Maps between application models and database tables
- `stmt/`: Statement AST nodes for queries, inserts, updates, deletes
- `driver/`: Abstract driver interface and operations

**Change Patterns**: 
- New data types require updating `schema/app/field/primitive.rs`
- Query features require new statement nodes in `stmt/`
- Driver operations must be added to `driver/operation/`

### 2. toasty-codegen (Code Generation Layer)
**Purpose**: Generates Rust code from the #[derive(Model)] macro.

**Key Components**:
- `schema/`: Parses model attributes and builds schema representation
- `expand/`: Generates implementations for models
  - `model.rs`: Core Model trait implementation
  - `query.rs`: Query builder methods
  - `create.rs`: Create/insert builders
  - `update.rs`: Update builders
  - `relation.rs`: Relationship methods

**Change Patterns**:
- New model features require updates to both schema parsing and expansion
- Query methods follow a builder pattern with method chaining
- Generated code must use fully qualified paths (e.g., `#toasty::Model`)

### 3. toasty (Main Library Layer)
**Purpose**: User-facing API, query engine, and runtime execution.

**Key Components**:
- `engine/`: Query planning and execution pipeline
  - `planner/`: Converts statements to execution plans
  - `exec/`: Executes plans against drivers
  - `simplify/`: Query optimization and simplification
  - `verify/`: Runtime validation
- `stmt/`: High-level statement builders
- `relation/`: Relationship abstractions (HasMany, BelongsTo, HasOne)
- `model.rs`: Model trait and ID generation

**Change Patterns**:
- Query features flow through: stmt → simplify → planner → exec
- New operations require plan nodes and execution logic
- Simplification rules optimize before planning

### 4. toasty-driver-* (Driver Layer)
**Purpose**: Database-specific implementations.

**Supported Databases**:
- `toasty-driver-sqlite`: SQLite implementation
- `toasty-driver-postgresql`: PostgreSQL implementation  
- `toasty-driver-mysql`: MySQL implementation
- `toasty-driver-dynamodb`: DynamoDB implementation

**Change Patterns**:
- Drivers implement the `driver::Driver` trait from toasty-core
- SQL drivers share common patterns through toasty-sql
- NoSQL drivers have custom operation implementations

### 5. toasty-sql (SQL Serialization Layer)
**Purpose**: Converts statement AST to SQL strings.

**Key Components**:
- `serializer/`: SQL generation with dialect support
  - `flavor.rs`: Database-specific SQL dialects
  - `params.rs`: Parameter binding strategies
- `stmt/`: SQL-specific statement types

**Change Patterns**:
- New SQL features require serializer updates
- Database-specific syntax handled via Flavor enum
- Parameter binding varies by database

## Common Change Patterns Across Components

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
4. Implement planning in `toasty/src/engine/planner/select.rs`
5. Add SQL serialization in `toasty-sql/src/serializer/stmt.rs`
6. Write integration tests

### Refactoring Patterns
- **Import cleanup**: Replace glob imports with specific imports
- **Dead code removal**: Remove unused functions and modules
- **Macro generation**: Use macros to reduce repetitive implementations
- **Type consolidation**: Merge similar types (e.g., ExprField + ExprReference)

## Key Design Decisions

### Dynamic Model IDs
Models use runtime-generated IDs via `generate_unique_id()` instead of compile-time constants. This allows for more flexible schema composition.

### Statement Simplification
Queries go through a simplification phase before planning to optimize and normalize the AST. This includes:
- Lifting subqueries
- Flattening boolean operations
- Rewriting path expressions

### Driver Capabilities
Drivers advertise capabilities (e.g., auto-increment support) which the planner uses to generate appropriate operations.

### Separation of App Schema and DB Schema
The application model structure can differ from the database schema, allowing multiple models to map to single tables or custom column mappings.

## Testing Strategy

### Test Organization
- Unit tests in each crate's `tests/` directory
- Integration tests in workspace `tests/` crate
- UI tests for compile-time error messages
- Database-specific test runners with isolation

### Test Patterns
- Tests use macros to run against all database drivers
- Isolation via temporary databases or table prefixes
- Concurrent test execution support