# Stored Procedures (Pre-Compiled Query Plans)

## Overview

Toasty re-compiles every query from scratch on each execution. The compilation pipeline — simplification, lowering, HIR planning, MIR planning, and execution planning — runs in full every time a user calls `.collect()`, `.exec()`, or similar methods, even when the query shape is identical and only the parameter values change.

Stored procedures let a query be compiled once and executed many times with different parameters. The compilation output (an `ExecPlan`) is cached, and subsequent calls substitute new parameter values into the pre-built plan without re-running the compiler.

The term "stored procedure" here refers to Toasty-side plan caching, not database-side stored procedures (though database-side prepared statements are a related optimization that could be layered on top).

## Problem

The current `Engine::exec` method runs five compilation phases on every call:

```
Statement AST → [Simplify] → [Lower to HIR] → [Plan to MIR] → [Execution Plan] → [Execute]
```

For a hot query like `User::find_by_id(id).collect(&db)`, the statement shape never changes — only the `id` value differs between calls. Re-running the full pipeline each time wastes CPU on work whose output is always the same modulo parameter values.

This matters most for:

- **High-throughput services** that execute the same query patterns thousands of times per second
- **Complex queries with includes** where the planning phase builds multi-step programs (subqueries, nested merges, batched loads) that are expensive to plan but identical across invocations
- **DynamoDB queries** where the planner must decompose subqueries into multiple operations — the decomposition is purely structural and does not depend on parameter values

## Design

### Parameterized Statements

A stored procedure separates the query *shape* from its *values*. The statement AST uses placeholder parameters instead of concrete values:

```rust
// Current: values are baked into the statement
let stmt = User::find_by_id(42);  // 42 is embedded in the AST

// Stored procedure: values are supplied at execution time
let find_user = storedproc!(User::find_by_id(?));
let user = find_user.exec(&db, (42,)).await?;
```

The compiler runs once on the parameterized statement and produces an `ExecPlan` with parameter slots. Execution fills those slots with concrete values and runs the interpreter.

### Compilation Caching

The stored procedure holds a compiled `ExecPlan` (or the intermediate form needed to produce one). On each call:

1. Clone or re-instantiate the plan template with new parameter values
2. Run the execution interpreter directly — skip simplification, lowering, and planning entirely

This keeps the per-call cost proportional to the number of database round-trips, not the complexity of the query shape.

### Parameter Slots in the Plan

The `ExecPlan` currently embeds concrete values in its actions (e.g., a `WHERE id = 42` filter has `42` baked into the `ExecStatement`). To support stored procedures, plans need a way to reference parameters by position:

- Introduce a `Param(usize)` variant to the expression AST, representing "the Nth argument supplied at execution time"
- The plan template contains `Param` references where values would normally appear
- At execution time, a parameter list resolves each `Param(N)` to its concrete value before the action runs

### What Gets Cached

The entire compilation output through the execution plan phase:

| Phase | Runs at compile time | Runs at execution time |
|-------|---------------------|----------------------|
| Simplification | Once | Never |
| Lowering to HIR | Once | Never |
| HIR → MIR planning | Once | Never |
| Execution planning | Once | Never |
| Parameter binding | — | Every call |
| Interpreter execution | — | Every call |

### API Sketch

```rust
// Define a stored procedure (compiled once)
let find_user = db.procedure(|p: Param<i64>| {
    User::find_by_id(p)
}).await?;

// Execute with different parameters (no re-compilation)
let alice = find_user.exec(&db, 1).await?;
let bob = find_user.exec(&db, 2).await?;

// Stored procedures with multiple parameters
let find_todos = db.procedure(|user_id: Param<i64>, status: Param<String>| {
    Todo::filter_by_user_id(user_id)
        .filter(Todo::FIELDS.status().eq(status))
}).await?;

let todos = find_todos.exec(&db, (1, "active".to_string())).await?;

// Stored procedures with includes (the include structure is compiled once)
let user_with_todos = db.procedure(|p: Param<i64>| {
    User::find_by_id(p).include(User::FIELDS.todos())
}).await?;

let user = user_with_todos.exec(&db, 42).await?;
```

### Macro-Based Alternative

A proc-macro could define stored procedures at compile time, moving the planning cost to `cargo build`:

```rust
#[toasty::stored_procedure]
fn find_user(id: i64) -> Query<User> {
    User::find_by_id(id)
}

// Usage — no runtime compilation
let user = find_user.exec(&db, 42).await?;
```

This requires the schema to be available at compile time (which it already is for `#[derive(Model)]`), but the driver capability is not known until runtime. The macro could generate the compilation pipeline output as a const, with a runtime check that the driver capability matches.

## Implementation Considerations

### Expression AST Changes

Add a parameter placeholder to the expression types in `toasty-core`:

**Files:**
- `crates/toasty-core/src/stmt/expr.rs` — Add `Expr::Param { index: usize, ty: Type }` variant
- `crates/toasty-core/src/stmt/visit.rs` and `visit_mut.rs` — Handle the new variant in visitors

The `Param` variant carries a type so that the compiler can type-check the plan without knowing the concrete value.

### Plan Template and Instantiation

The `ExecPlan` struct needs a way to accept parameter values and substitute them into the action sequence:

**Files:**
- `crates/toasty/src/engine/exec/plan.rs` — Add parameter metadata (count, types) to `ExecPlan`
- `crates/toasty/src/engine/exec.rs` — Add a parameter-binding step before the interpreter loop

The binding step walks the action list and replaces `Param` references with concrete values from the argument tuple. This is a shallow pass over the plan — no tree rewriting, just slot filling.

### Stored Procedure Handle

A new public type that owns the compiled plan and exposes an `exec` method:

**Files:**
- `crates/toasty/src/stored_procedure.rs` — `StoredProcedure<Args, Output>` type
- `crates/toasty/src/db.rs` — `Db::procedure()` method for creating stored procedures

The handle is `Clone + Send + Sync` so it can be shared across tasks and threads. The compiled plan inside is immutable; each `exec` call creates a short-lived copy with bound parameters.

### Driver-Side Prepared Statements

Stored procedures at the Toasty level pair naturally with database-level prepared statements. Once the plan is fixed, the SQL strings it generates are also fixed — they can be prepared once and executed with bound parameters:

- SQL drivers: Use `PREPARE` / `EXECUTE` or the database client's prepared statement API
- DynamoDB: No equivalent, but the operation structure is already fixed

This is a separate optimization that layers on top of plan caching. The stored procedure provides the stable SQL strings that prepared statements need.

### Invalidation

A stored procedure is valid as long as the schema and driver capability remain the same. Since Toasty schemas are defined at compile time and don't change at runtime, invalidation is not a concern for the initial implementation. If schema migration support is added later, stored procedures would need to be re-compiled after a migration.

## Interaction with Other Features

**Query caching (Performance roadmap):** Query caching and stored procedures address different costs. Query caching avoids the database round-trip by reusing previous results. Stored procedures avoid the compilation cost but still hit the database. They compose well — a cached result eliminates both costs; a stored procedure reduces cost when the cache misses.

**Concurrent task execution (Runtime roadmap):** The execution interpreter could run independent actions in parallel. Stored procedures don't change this — the compiled plan is the same structure whether it was just compiled or retrieved from cache. Concurrent execution applies equally to both.

**Raw SQL support (Query Building roadmap):** Raw SQL fragments embedded in a stored procedure must also be parameterized. The `Param` mechanism extends naturally to raw SQL: `toasty::raw_sql!("custom_func(?, ?)", p1, p2)` would place `Param` references that get bound at execution time.
