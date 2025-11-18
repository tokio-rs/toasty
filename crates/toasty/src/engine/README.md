# Toasty Query Engine

This document provides a high-level overview of the Toasty query execution
engine for developers working on engine internals. It describes the multi-phase
pipeline that transforms user queries into database operations.

## Overview

The Toasty engine is a multi-database query compiler and runtime that executes
ORM operations across SQL and NoSQL databases. It uses multiple intermediate
representations (IRs) and optimization passes:

```
User Query (Statement AST)
    ↓
[Verification] - Validate statement structure (debug builds only)
    ↓
[Simplification] - Normalize and optimize the statement AST
    ↓
[Lowering] - Convert to HIR for dependency analysis
    ↓
[Planning] - Build MIR operation graph (database operations and in-memory transforms)
    ↓
[Execution Planning] - Convert to executable action sequence
    ↓
[Execution] - Execute against database driver
    ↓
Result Stream
```

## Phase 1: Simplification

**Location**: `engine::simplify`

The simplification phase normalizes and optimizes the statement AST before planning.

### Key Transformations

- **Association Rewriting**: Converts relationship navigation (e.g., `user.todos()`) into explicit subqueries with foreign key filters
- **Subquery Lifting**: Transforms `IN (SELECT ...)` expressions into more efficient join-like operations
- **Expression Normalization**: Simplifies complex expressions (e.g., flattening nested ANDs/ORs, constant folding)
- **Path Expression Rewriting**: Resolves field paths and relationship traversals into explicit column references
- **Empty Query Detection**: Identifies queries that will return no results

### Example: Association Simplification

```rust
// user.todos().delete() generates:

Delete {
    from: Todo,
    via: User::todos,  // relationship traversal
    ...
}

// After simplification:

Delete {
    from: Todo,
    filter: todo.user_id IN (SELECT id FROM users WHERE ...)
}
```

Converting relationship navigation into explicit filters early means downstream phases only need to handle standard query patterns with filters and subqueries - no special-case logic for each relationship type.

## Phase 2: Lowering

**Location**: `engine::lower`

Lowering converts a simplified statement into **HIR (High-level Intermediate
Representation)** - a collection of related statements with tracked
dependencies.

Toasty tries to maximize what the target database can handle natively, only decomposing queries when necessary. For example, a query like `User::find_by_name("John").todos().all()` contains a subquery. SQL databases can execute this as `SELECT * FROM todos WHERE user_id IN (SELECT id FROM users WHERE name = 'John')`. DynamoDB cannot handle subqueries, so lowering splits this into two statements: first fetch user IDs, then query todos with those IDs. The HIR tracks that the second statement depends on the first.

This phase handles:

- **Statement Decomposition**: Breaking queries into sub-statements when the database can't handle them directly
- **Dependency Tracking**: Which statements must execute before others
- **Argument Extraction**: Identifying values passed between statements (e.g., a loaded model's ID used in a child query's filter)
- **Relationship Handling**: Processing relationship loads and nested queries

## Phase 3: Planning

**Location**: `engine::plan`

Planning converts HIR into **MIR (Middle-level Intermediate Representation)** - a directed acyclic graph of operations, both database queries and in-memory transformations. Edges represent data dependencies: an operation cannot execute until all operations it depends on have completed and produced their results.

### Operation Types

The MIR supports various operation types (`mir/operation.rs`):

- **`ExecStatement`**: Execute a database query (SELECT, INSERT, UPDATE, DELETE)
- **`GetByKey`**: Fetch records by primary key
- **`QueryPk`**: Query primary keys only (for efficient lookups)
- **`FindPkByIndex`**: Use an index to find primary keys
- **`Filter`**: In-memory filtering of results
- **`Project`**: Transform/extract fields from records
- **`NestedMerge`**: Merge child records into parent records (for relationships)
- **`DeleteByKey`**: Delete records by primary key
- **`UpdateByKey`**: Update records by primary key
- **`ReadModifyWrite`**: Conditional updates (optimistic locking)
- **`Const`**: Constant value

### Planning Algorithm

The planner works in two stages:

1. **HIR to MIR** (`HirPlanner`): Converts each HIR statement into MIR operations
   - Determines which columns to select
   - Creates operations for sub-statements
   - Sets up batch-loading for nested relationships
   - Handles back-references between statements

2. **MIR Optimization**: Orders operations for efficient execution
   - Topological sort for dependency ordering
   - Reference counting for result caching

### Nested Merge Pattern

For queries that load relationships (e.g., `user.todos`), the planner creates a pattern:

```
1. QueryPk(users WHERE ...)          → List of user IDs
2. GetByKey(users, user_ids)         → Parent records
3. ExecStatement(SELECT todos WHERE user_id IN (...))  → Child records
4. NestedMerge(parents, children)    → Combined result
```

**TODO**: Document batch loading optimization strategy

**TODO**: Explain index-aware planning (when to use `FindPkByIndex`)

**TODO**: Document how the planner handles different database capabilities (SQL vs NoSQL)

## Phase 4: Execution Planning

**Location**: `engine/plan/execution.rs`

Execution planning converts the MIR logical plan into a concrete sequence of **actions** that can be executed. This phase:

- Allocates variables for storing intermediate results
- Converts each MIR `Node` into an `exec::Action`
- Maintains topological ordering from the logical plan

The output is an `ExecPlan` containing:

```rust
struct ExecPlan {
    vars: VarStore,           // Variable storage for intermediate results
    actions: Vec<Action>,     // Ordered sequence of actions
    returning: Option<VarId>, // Variable containing final result
}
```

### Action Types

Actions mirror MIR operations but include concrete variable bindings:

- **`ExecStatement`**: Execute a database statement, store result in a variable
- **`GetByKey`**: Batch fetch by primary key
- **`Filter`**: Apply in-memory filter to a variable's data
- **`Project`**: Transform records
- **`NestedMerge`**: Merge nested data into parent records
- **`SetVar`**: Set a variable value
- etc.

## Phase 5: Execution

**Location**: `engine/exec.rs` and `engine/exec/*`

The execution phase runs the `ExecPlan` against the database driver. This is an async operation that:

1. Initializes variable storage
2. Executes each action in sequence
3. Stores intermediate results in variables
4. Returns the final result stream

### Execution Context

```rust
struct Exec<'a> {
    engine: &'a Engine,
    vars: VarStore,  // Runtime variable storage
}
```

### Variable Management

The `VarStore` manages intermediate results during execution:

- Variables are lazily loaded (only when first accessed)
- Results are cached for reuse if referenced multiple times
- Supports both value streams and row counts

### Action Execution

Each action type has a corresponding `action_*` method that:

1. Loads input data from variables
2. Performs the operation (database query, in-memory transform, etc.)
3. Stores the result in the output variable

### Driver Interaction

The execution phase is the only part that directly communicates with database drivers. Key operations:

- **`driver.exec()`**: Execute a statement, return rows
- **`driver.get_by_key()`**: Fetch records by primary key
- **`driver.find_pk_by_index()`**: Use index to find keys

**TODO**: Document error handling and transaction semantics

**TODO**: Explain streaming vs. materialization tradeoffs

## Supporting Components

### Type System

**Location**: `engine/ty.rs`

Tracks the types of expressions and intermediate results throughout the pipeline.

### Expression Evaluation

**Location**: `engine/eval.rs` and `engine/eval/*`

Evaluates expressions in memory when needed (e.g., for post-filtering, projections).

### Index Analysis

**Location**: `engine/index.rs`

Analyzes queries to determine when indexes can be used for efficient lookups.

**TODO**: Document index selection algorithm

### Key-Value Operations

**Location**: `engine/kv.rs`

Utilities for generating primary key operations (get by key, delete by key, etc.).

### Verification

**Location**: `engine/verify.rs`

Debug-mode validation of statement structure and semantics:

- Ensures filter expressions are boolean
- Validates offset keys match ORDER BY clauses
- Verifies field references resolve correctly

## Optimization Strategies

The engine employs several optimization techniques across phases:

1. **Simplification-time**:
   - Constant folding
   - Dead expression elimination
   - Subquery lifting
   - Boolean expression flattening

2. **Planning-time**:
   - Batch loading of relationships (N+1 query prevention)
   - Index-aware query generation
   - Primary key extraction for efficient lookups
   - Query result reuse via reference counting

3. **Execution-time**:
   - Lazy evaluation of intermediate results
   - Result streaming where possible
   - Minimal materialization

**TODO**: Document specific optimization patterns with examples

**TODO**: Explain how the engine avoids N+1 queries

## Database Driver Abstraction

The engine supports both SQL and NoSQL databases through a unified driver interface. Key differences:

### SQL Databases

- Statements are serialized to SQL strings (`toasty-sql` crate)
- Complex queries executed in a single database roundtrip
- Server-side filtering and joins

### NoSQL Databases (e.g., DynamoDB)

- Operations decomposed into primitive key-value operations
- More client-side filtering and merging
- Multiple roundtrips for complex queries

The planner adapts its strategy based on `driver.capability()`.

**TODO**: Document capability-based planning decisions

## Performance Characteristics

### Allocation Behavior

- Simple queries make no allocations
- Relationship loading allocates for batch collection
- Results stream without buffering

### Trade-offs

- **Compilation cost**: Multi-phase pipeline has overhead per query
- **Memory usage**: Intermediate representations require allocation (optimized via arenas)
- **Roundtrips**: NoSQL queries may require multiple database calls

**TODO**: Add benchmarking data and optimization guidelines

## Debugging

### Statement Inspection

Use `{:#?}` formatting on statements to see their structure at any phase:

```rust
eprintln!("After simplification: {stmt:#?}");
```

### Execution Tracing

Add debug prints in action handlers to trace execution:

```rust
dbg!(&action);
```

### Common Issues

1. **Empty result sets**: Check simplification phase - may be optimizing away necessary logic
2. **Missing columns**: Verify back-ref tracking in lowering phase
3. **Incorrect ordering**: Check execution_order in logical plan
4. **N+1 queries**: Verify NestedMerge operations are being generated

**TODO**: Document common debugging patterns and tools

## Future Work

Areas for potential improvement:

- [ ] Query result caching
- [ ] Parallel operation execution (where safe)
- [ ] More aggressive index usage
- [ ] Query plan visualization tools
- [ ] Cost-based optimization
- [ ] Prepared statement pooling
- [ ] Incremental query compilation

## See Also

- `docs/ARCHITECTURE.md` - Overall Toasty architecture
- `toasty-core/src/stmt/` - Statement AST definitions
- `toasty-sql/` - SQL serialization
- `toasty-driver-*/` - Database driver implementations
