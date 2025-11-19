# Toasty Query Engine

This document provides a high-level overview of the Toasty query execution
engine for developers working on engine internals. It describes the multi-phase
pipeline that transforms user queries into database operations.

## Overview

The Toasty engine is a multi-database query compiler and runtime that executes ORM operations across SQL and NoSQL databases. It transforms a user's query (represented as a Statement AST) into a sequence of executable actions through multiple compilation phases.

### Execution Model

The final output is a **mini program** executed by an interpreter. Think of it like a small virtual machine or bytecode interpreter, though there is no control flow (yet):

- **Instructions (Actions)**: Operations like "execute this SQL", "filter these results", "merge child records into parents"
- **Variables**: Storage slots, or registers, that hold intermediate results between instructions
- **Linear Execution**: Instructions run in sequence (no control flow - no branches or loops, yet). Eventually, the interpreter will be smart about concurrency and execute independent operations in parallel when possible.
- **Interpreter**: The engine executor reads each instruction, fetches inputs from variables, performs the operation, and stores outputs back to variables

For example, loading users with their todos:

```sql
SELECT users.id, users.name, (
    SELECT todos.id, todos.title 
    FROM todos 
    WHERE todos.user_id = users.id
) FROM users WHERE ...
```

compiles to a program like:

```
$0 = ExecSQL("SELECT * FROM users WHERE ...")
$1 = ExecSQL("SELECT * FROM todos WHERE user_id IN ...")
$2 = NestedMerge($0, $1, by: user_id)
return $2
```

The compilation pipeline below transforms user queries into this instruction/variable representation. Each phase brings the query closer to this final executable form.

### Compilation Pipeline

```
User Query (Statement AST)
    ↓
[Verification] - Validate statement structure (debug builds only)
    ↓
[Simplification] - Normalize and optimize the statement AST
    ↓
[Lowering] - Convert to HIR for dependency analysis
    ↓
[Planning] - Build MIR operation graph
    ↓
[Execution Planning] - Convert to action sequence with variables
    ↓
[Execution] - Run actions against database driver
    ↓
Result Stream
```

## Phase 1: Simplification

**Location**: `engine/simplify.rs`

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

**Location**: `engine/lower.rs`

Lowering converts a simplified statement into **HIR (High-level Intermediate
Representation)** - a collection of related statements with tracked
dependencies.

Toasty tries to maximize what the target database can handle natively, only decomposing queries when necessary. For example, a query like `User::find_by_name("John").todos().all()` contains a subquery. SQL databases can execute this as `SELECT * FROM todos WHERE user_id IN (SELECT id FROM users WHERE name = 'John')`. DynamoDB cannot handle subqueries, so lowering splits this into two statements: first fetch user IDs, then query todos with those IDs.

The HIR tracks a dependency graph between statements - which statements depend on results from others, and which columns flow between them. This graph can contain cycles when preloading associations. For example:

```sql
SELECT users.id, users.name, (
    SELECT todos.id, todos.title 
    FROM todos 
    WHERE todos.user_id = users.id
) FROM users WHERE ...
```

The users query must execute first to provide IDs for the todos subquery, but the todos results must be merged back into the user records. This creates a cycle: users → todos → users.

This lowering phase handles:

- **Statement Decomposition**: Breaking queries into sub-statements when the database can't handle them directly
- **Dependency Tracking**: Which statements must execute before others
- **Argument Extraction**: Identifying values passed between statements (e.g., a loaded model's ID used in a child query's filter)
- **Relationship Handling**: Processing relationship loads and nested queries

### Lowering Algorithm

Lowering transforms model-level statements to table-level statements through a visitor pattern that rewrites each part of the statement AST:

1. **Table Resolution**: `InsertTarget::Model`, `UpdateTarget::Model`, etc. become their corresponding table references
2. **Returning Clause Transformation**: `Returning::Model` is replaced with `Returning::Expr` containing the expanded column expressions
3. **Field Reference Resolution**: Model field references are converted to table column references
4. **Include Expansion**: Association includes become subqueries in the returning clause

The `TableToModel` mapping (built during schema construction) drives the transformation. It contains an expression for each model field that maps to its corresponding table column(s). This supports more than a 1-1 mapping—a model field can be derived from multiple columns or a column can map to multiple fields. Association fields are initialized to `Null` in this mapping.

When lowering encounters a `Returning::Model { include }` clause:
1. Call `table_to_model.lower_returning_model()` to get the base column expressions
2. For each path in the include list, call `build_include_subquery()` to generate a subquery that selects the associated records
3. Replace the `Null` placeholder in the returning expression with the generated subquery

### Lowering Examples

**Example 1: Simple query**

Given a model with a renamed column:
```rust
#[derive(Model)]
struct User {
    #[key] id: Id<Self>,
    #[column(name = "first_and_last_name")]
    name: String,
    email: String,
}
```

```rust
// Before lowering (toasty_core::stmt::Statement)
SELECT MODEL FROM User WHERE id = ?
// Note: At model-level, no specific fields are selected

// After lowering
SELECT id, first_and_last_name, email FROM users WHERE id = ?
```

**Example 2: Query with association**

```rust
// Before lowering (toasty_core::stmt::Statement)
SELECT MODEL FROM User WHERE id = ?
  INCLUDE todos

// After lowering
SELECT id, first_and_last_name, email, (
    SELECT id, title, user_id FROM todos WHERE todos.user_id = users.id
) FROM users WHERE id = ?
```

## Phase 3: Planning

**Location**: `engine/plan.rs`

Planning converts HIR into **MIR (Middle-level Intermediate Representation)** - a directed acyclic graph of operations, both database queries and in-memory transformations. Edges represent data dependencies: an operation cannot execute until all operations it depends on have completed and produced their results.

Since the HIR graph can contain cycles, planning must break them to produce a DAG. This is done by introducing intermediate operations that batch-load data and merge results (e.g., `NestedMerge`).

### Operation Types

The MIR supports various operation types (see `engine/mir.rs` for details):

**SQL operations:**
- `ExecStatement` - Execute a SQL query (SELECT, INSERT, UPDATE, DELETE)
- `ReadModifyWrite` - Optimistic locking (read, modify, conditional write). Exists as a separate operation because the read result must be processed in-memory to compute the write, which `ExecStatement` cannot express.

**Key-value operations (NoSQL):**
- `GetByKey`, `DeleteByKey`, `UpdateByKey` - Direct key access
- `QueryPk`, `FindPkByIndex` - Key lookups via queries or indexes

**In-memory operations:**
- `Filter`, `Project` - Transform and filter results
- `NestedMerge` - Merge child records into parent records
- `Const` - Constant values

## Phase 4: Execution Planning

**Location**: `engine/plan/execution.rs`

Execution planning converts the MIR logical plan into a concrete sequence of **actions** that can be executed. This phase:

- Assigns variable slots for storing intermediate results
- Converts each MIR operation into an execution action
- Maintains topological ordering to ensure dependencies execute first

### Action Types

Actions mirror MIR operations but include concrete variable bindings:

**SQL actions:**
- **`ExecStatement`**: Execute a SQL query (SELECT, INSERT, UPDATE, DELETE)
- **`ReadModifyWrite`**: Optimistic locking (read, modify, conditional write)

**Key-value actions (NoSQL):**
- **`GetByKey`**: Batch fetch by primary key
- **`DeleteByKey`**: Delete records by primary key
- **`UpdateByKey`**: Update records by primary key
- **`QueryPk`**: Query primary keys
- **`FindPkByIndex`**: Find primary keys via secondary index

**In-memory actions:**
- **`Filter`**: Apply in-memory filter to a variable's data
- **`Project`**: Transform records
- **`NestedMerge`**: Merge child records into parent records
- **`SetVar`**: Set a variable to a constant value

## Phase 5: Execution

**Location**: `engine/exec.rs`

The execution phase is the interpreter that runs the compiled program. It iterates through actions, reading inputs from variables, performing operations, and storing outputs back to variables.

### Execution Loop

The interpreter follows a simple pattern:

1. Initialize variable storage
2. For each action in sequence:
   - Load input data from variables
   - Perform the operation (database query or in-memory transform)
   - Store the result in the output variable
3. Return to the user the result from the final variable (the last action's output)

### Variable Lifetime

The engine tracks how many times each variable is referenced by downstream actions. A variable may be used by multiple actions (e.g., the same user records merged with both todos and comments). When the last action that needs a variable completes, the variable's value is dropped to free memory.

### Driver Interaction

The execution phase is the only part of the engine that communicates with database drivers. The driver interface is intentionally simple: a single `exec()` method that accepts an `Operation` enum. This enum includes variants for both SQL operations (`QuerySql`, `Insert`) and key-value operations (`GetByKey`, `QueryPk`, `FindPkByIndex`, `DeleteByKey`, `UpdateByKey`).

Each driver implements whichever operations it supports. SQL drivers handle `QuerySql` natively while key-value drivers handle `GetByKey`, `QueryPk`, etc. The planner uses `driver.capability()` to determine which operations to generate for each database type.
