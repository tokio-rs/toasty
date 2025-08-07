# Toasty Architecture Guide

## Overview

Toasty is a multi-database ORM for Rust that provides a unified API across SQL (PostgreSQL, MySQL, SQLite) and NoSQL (DynamoDB) databases. This document describes the internal architecture and design patterns that enable this flexibility.

## Core Design Principles

1. **Database Abstraction**: Single API that adapts to different database capabilities
2. **Type Safety**: Compile-time guarantees through Rust's type system and procedural macros
3. **Performance**: Capability-driven query planning for optimal database utilization
4. **Flexibility**: Support for both SQL and NoSQL paradigms without compromising features

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    User Application                      │
└─────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────┐
│                    Toasty ORM API                        │
│              (#[derive(Model)], CRUD methods)            │
└─────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────┐
│                    Toasty Engine                         │
│          (Query Planning & Statement Transformation)     │
└─────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────┐
│                   Database Drivers                       │
│     (SQLite, PostgreSQL, MySQL, DynamoDB)               │
└─────────────────────────────────────────────────────────┘
```

## Workspace Structure

```
toasty/
├── crates/
│   ├── toasty/              # Main user-facing ORM library
│   ├── toasty-core/         # Core abstractions (schema, drivers, statements)
│   ├── toasty-macros/       # Procedural macros for #[derive(Model)]
│   ├── toasty-codegen/      # Code generation logic
│   ├── toasty-sql/          # SQL statement generation
│   └── toasty-driver-*/     # Database-specific drivers
└── tests/                   # Integration tests across all databases
```

## The Engine: Heart of Toasty

The engine (`crates/toasty/src/engine`) is the core component that transforms high-level ORM operations into database-specific commands. It implements a sophisticated multi-stage pipeline that handles the complexity of supporting diverse database backends.

### Statement System: SQL Superset

Toasty uses an innovative "SQL superset" approach for its internal statement representation:

- **Toasty Statements** (`toasty_core::stmt::Statement`): Include both SQL concepts AND application-level concepts
  - SQL elements: SELECT, INSERT, UPDATE, DELETE, columns, tables, joins
  - Application elements: Models, fields, relationships, pagination cursors
- **Critical Invariant**: Statements must be transformed to pure SQL before reaching the SQL generator

### Four-Stage Execution Pipeline

#### 1. Verification Stage
- **Location**: `crates/toasty/src/engine/verify.rs`
- **Purpose**: Validate statement correctness against schema
- **When**: Debug builds only (`cfg!(debug_assertions)`)
- **Example**: Ensures field references exist in models, validates offset keys match ORDER BY

#### 2. Planning Stage
- **Location**: `crates/toasty/src/engine/planner/`
- **Purpose**: Generate execution plan based on database capabilities
- **Key Components**:
  - `Planner`: Orchestrates plan generation
  - `VarTable`: Manages intermediate result variables  
  - `Pipeline`: Sequence of actions with optional return value
  - `Action`: Atomic execution units
  - `eval::Func`: Pre-computed transformations for runtime evaluation

**Planning Process**:
1. **Capability Detection**: Check database features via `driver.capability()`
2. **Path Selection**: Choose SQL vs KV strategy
3. **Statement Partitioning**: Separate database operations from in-memory operations
4. **Projection Building**: Create `eval::Func` for result transformation
5. **Action Generation**: Build pipeline of execution steps

**Example Planning Decision**:
```rust
// SQL database: Use optimized SQL query
if capability.sql {
    plan_select_sql(...)  // Generates ExecStatement action
} else {
    // NoSQL: Use key-value operations with client-side filtering
    plan_select_kv(...)   // Generates GetByKey/QueryPk actions
}
```

#### 3. Simplification Stage
- **Location**: `crates/toasty/src/engine/simplify.rs`
- **Purpose**: Transform application concepts to database concepts
- **Key Transformations**:
  - Model references → Table references
  - Field paths → Column expressions
  - Relationship traversal (`via`) → JOIN operations
  - Application-level pagination → SQL LIMIT/WHERE clauses

#### 4. Execution Stage
- **Location**: `crates/toasty/src/engine/exec/`
- **Purpose**: Execute the plan and return results
- **Key Components**:
  - `VarStore`: Stores intermediate results between pipeline steps
  - `ExecResponse`: Contains both values and optional metadata
  - Action executors: Specialized handlers for each action type

**Execution Flow**:
1. Initialize `VarStore` with input variables
2. Execute each action in pipeline sequence
3. Actions read inputs from `VarStore` and write outputs back
4. Final result loaded from `VarStore` and returned

### Action System

The engine decomposes operations into atomic actions:

```rust
pub enum Action {
    Associate(Associate),           // Load relationships
    BatchWrite(BatchWrite),         // Batch operations
    DeleteByKey(DeleteByKey),       // Key-based deletion
    ExecStatement(ExecStatement),   // SQL execution
    FindPkByIndex(FindPkByIndex),   // Index lookup
    GetByKey(GetByKey),            // Key retrieval
    Insert(Insert),                // Record insertion
    QueryPk(QueryPk),              // Primary key query
    UpdateByKey(UpdateByKey),      // Key-based update
}
```

### eval::Func System

The `eval::Func` system enables pre-computed transformations during planning that execute during the pipeline:

**Purpose**: Separate database operations from in-memory transformations
**Location**: `crates/toasty/src/engine/eval.rs`

**Key Concepts**:
- **Input Types**: Defined at planning time from schema
- **Return Type**: Inferred or explicitly specified
- **Expression**: The transformation to apply

**Common Patterns**:

1. **Identity Function**: Pass through unchanged
```rust
eval::Func::identity(type)
```

2. **Projection**: Extract specific fields
```rust
// Extract field at index 2 from arg 0
stmt::Expr::arg_project(0, [2])
```

3. **Record Construction**: Build composite values
```rust
stmt::Expr::record_from_vec(vec![field1, field2])
```

### partition_returning Pattern

The planner uses `partition_returning` to handle complex SELECT clauses:

**Location**: `crates/toasty/src/engine/planner/output.rs`

**Process**:
1. **Analyze returning clause**: Determine what database can handle vs. in-memory evaluation
2. **Partition expressions**:
   - `Stmt`: Must be evaluated by database (e.g., column references)
   - `Eval`: Must be evaluated in-memory (e.g., complex transformations)
   - `ConstStmt`: Can be evaluated by either (e.g., constants)
3. **Modify statement**: Include only database-evaluable expressions
4. **Build eval::Func**: Create transformation for in-memory evaluation

**Example**:
```rust
// Original returning clause
returning: User { 
    id: column("id"),
    name: upper(column("name")),  // Complex transformation
    email: column("email")
}

// After partitioning
stmt.returning: [column("id"), column("name"), column("email")]
eval::Func: record([
    arg_project(0, [0]),           // id
    upper(arg_project(0, [1])),    // transformed name
    arg_project(0, [2])            // email
])
```

## Schema Architecture

Toasty uses a three-layer schema system to bridge application models and database tables:

### 1. Application Schema (`app::Schema`)
- User-defined models and relationships
- High-level abstractions (e.g., `User`, `Post`)
- Generated from `#[derive(Model)]`

### 2. Database Schema (`db::Schema`)
- Physical database structure
- Tables, columns, indexes
- Database-specific details

### 3. Mapping Layer
- Translates between application and database schemas
- Handles naming conventions
- Manages type conversions

**Example Translation**:
```rust
// Application level
User::FIELDS.email()
// ↓ Mapping
// Database level
Column("users", "email")
```

## Driver System

### Driver Trait
- **Location**: `crates/toasty-core/src/driver.rs`
- **Purpose**: Database abstraction interface
- **Key Method**: `exec(schema, operation) -> Response`

### Capability System
Drivers advertise their capabilities to enable optimized query planning:

```rust
pub struct Capability {
    sql: bool,                      // SQL support
    storage_types: StorageTypes,    // Supported data types
    cte_with_update: bool,          // PostgreSQL CTEs
    select_for_update: bool,        // Row locking
    primary_key_ne_predicate: bool, // DynamoDB limitation
}
```

### Operation Types
```rust
pub enum Operation {
    QuerySql(QuerySql),            // SQL queries
    GetByKey(GetByKey),            // Key-value retrieval
    Insert(Insert),                // Record insertion
    UpdateByKey(UpdateByKey),      // Key-based update
    DeleteByKey(DeleteByKey),      // Key-based deletion
    FindPkByIndex(FindPkByIndex),  // Index lookup
}
```

## VarStore and Pipeline Execution

The VarStore is the heart of pipeline execution, enabling data flow between actions:

### VarStore Architecture
- **Purpose**: Store intermediate results during pipeline execution
- **Structure**: Vector of optional `ExecResponse` values indexed by `VarId`
- **Operations**:
  - `store(var_id, response)`: Save result from an action
  - `load(var_id)`: Take ownership of stored result
  - `dup(var_id)`: Clone a stored result (for multiple consumers)

### Pipeline Execution Flow

```rust
// Example pipeline for: User::filter_by_email("alice@example.com")
Pipeline {
    actions: [
        ExecStatement {
            stmt: SELECT * FROM users WHERE email = ?,
            input: Some(Input { source: Value(0) }),  // Email parameter
            output: Some(Output { 
                var: 1,
                project: identity()  // No transformation needed
            })
        }
    ],
    returning: Some(1)  // Return contents of var 1
}
```

**Execution Steps**:
1. VarStore initialized with input (var 0 = "alice@example.com")
2. ExecStatement reads var 0 as input parameter
3. Database query executes
4. Result stored in var 1 with any projections applied
5. Pipeline returns contents of var 1

### Complex Pipeline Example

For queries with relationships or multiple steps:

```rust
// User::get_by_id(1).posts()
Pipeline {
    actions: [
        GetByKey {           // Step 1: Get user
            key: [1],
            output: Output { var: 1, project: user_projection }
        },
        Associate {          // Step 2: Load posts
            input: Input { source: Value(1) },  // User from step 1
            association: "posts",
            output: Output { var: 2, project: posts_projection }
        }
    ],
    returning: Some(2)  // Return posts
}
```

## Query Transformation Examples

### Example 1: Pagination

**User Code**:
```rust
User::all()
    .order_by(User::FIELDS.created_at.desc())
    .paginate(10)
    .after(cursor)
```

**Transformation Pipeline**:

1. **Initial Statement** (Application-level):
```rust
Query {
    source: Model("User"),
    limit: PaginateForward { limit: 10, after: cursor },
    order_by: Field(User::created_at)
}
```

2. **After Planning** (Mixed):
```rust
Query {
    source: Model("User"),
    limit: Offset { limit: 11 },  // +1 for has_more detection
    filter: created_at > cursor_value,
    order_by: Field(User::created_at)
}
```

3. **After Simplification** (Database-level):
```rust
Query {
    source: Table("users"),
    limit: Offset { limit: 11 },
    filter: Column("created_at") > Value,
    order_by: Column("created_at")
}
```

4. **Generated SQL**:
```sql
SELECT * FROM users 
WHERE created_at > ? 
ORDER BY created_at DESC 
LIMIT 11
```

### Example 2: Relationship Loading

**User Code**:
```rust
user.posts()
    .filter(Post::FIELDS.published.eq(true))
    .all()
```

**Transformation Pipeline**:

1. **Initial**: Model-based query with relationship
2. **Planning**: Determine join vs separate query strategy
3. **Simplification**: Convert to table join or foreign key lookup
4. **SQL Generation**: Produce optimized SQL

## Code Generation System

### Procedural Macros
- **Location**: `crates/toasty-macros`
- **Purpose**: Generate CRUD methods from model definitions
- **Process**: Parse → Analyze → Generate

### Generated Methods
For each model, the macro generates:
- `create()` - Insert new records
- `get_by_*()` - Retrieve by fields
- `filter_by_*()` - Query by fields
- `update()` - Modify records
- `delete()` - Remove records
- Relationship navigation methods

**Example Generated Code**:
```rust
impl User {
    pub async fn create(self) -> Result<User> { ... }
    pub async fn get_by_id(db: &Db, id: i64) -> Result<User> { ... }
    pub fn filter_by_email(email: &str) -> Filter<User> { ... }
    pub async fn update(self) -> Result<()> { ... }
    pub async fn delete(self) -> Result<()> { ... }
    pub fn posts(&self) -> Query<Post> { ... }
}
```

## Key Design Patterns

### 1. Visitor Pattern
Used for statement tree traversal and transformation:
```rust
impl Visit for MyVisitor {
    fn visit_expr(&mut self, expr: &Expr) {
        // Transform expression
    }
}
```

### 2. Builder Pattern
Fluent API for query construction:
```rust
User::all()
    .filter(...)
    .order_by(...)
    .limit(...)
```

### 3. Type State Pattern
Compile-time guarantees for query building:
```rust
Query<User>           // Type-safe query
Filter<User>          // Type-safe filter
Update<User>          // Type-safe update
```

### 4. Strategy Pattern
Database-specific behavior through capability dispatch:
```rust
match capability {
    Capability { sql: true, .. } => sql_strategy(),
    Capability { sql: false, .. } => nosql_strategy(),
}
```

## Performance Optimizations

### 1. Capability-Driven Planning
- Query plans adapt to database features
- SQL databases: Native SQL optimization
- NoSQL databases: Efficient key-value operations

### 2. Batch Operations
- Group multiple operations when possible
- Reduce round trips to database
- Transaction support where available

### 3. Lazy Loading
- Relationships loaded on-demand
- Configurable eager loading
- N+1 query prevention

### 4. Statement Caching
- Prepared statements for SQL databases
- Query plan caching
- Connection pooling

## Error Handling

### Error Types
- **Schema Errors**: Model definition issues
- **Validation Errors**: Data constraint violations
- **Driver Errors**: Database-specific errors
- **Planning Errors**: Unsupported operations

### Error Propagation
- Uses `anyhow::Result<T>` throughout
- Errors bubble up with context
- Debug mode includes detailed traces

## Testing Strategy

### Test Structure
```
tests/
├── tests/           # Integration tests
│   ├── crud.rs     # CRUD operations
│   ├── query.rs    # Query tests
│   └── ...
└── fixtures/       # Test data and models
```

### Multi-Database Testing
Tests run against all enabled backends:
```bash
cargo test --features sqlite
cargo test --features postgresql
cargo test --features mysql
cargo test --features dynamodb
```

### Test Macros
```rust
tests! {
    async fn test_name(db: Db) {
        // Test runs for each database
    }
}
```

## Development Guidelines

### Adding New Features

1. **Define in Statement System**: Add to `toasty_core::stmt`
2. **Update Planner**: Handle in `engine/planner.rs`
3. **Add Simplification**: Transform in `engine/simplify.rs`
4. **Update Drivers**: Implement in each driver
5. **Add Tests**: Cover all databases

### Debugging Tips

1. **Enable Debug Logging**: `RUST_LOG=debug`
2. **Check Verification**: Errors in debug builds
3. **Inspect Plans**: Log generated actions
4. **Trace SQL**: Log generated SQL statements

### Common Pitfalls

1. **Statement Invariants**: Ensure application concepts are removed before SQL generation
2. **Schema Consistency**: Keep app and db schemas synchronized
3. **Driver Capabilities**: Test operations against all backends
4. **Async Context**: Remember all operations are async

## Future Directions

### Planned Features
- Transaction support across all backends
- Advanced query optimization
- Query result caching
- Migration system
- Performance monitoring

### Architecture Evolution
- Plugin system for custom drivers
- Query optimizer improvements
- Streaming result support
- Distributed database support

## Conclusion

Toasty's architecture achieves database abstraction through:
- A sophisticated statement transformation pipeline
- Capability-driven query planning
- Clean separation between application and database concerns
- Type-safe code generation

This design enables a single, ergonomic API that works efficiently across diverse database backends while maintaining the flexibility to optimize for each database's strengths.