# Toasty Type System Architecture

## Overview

Toasty uses Rust's type system in the public API with both concrete types and generics. The query engine tracks the type of value each statement evaluates to using `stmt::Type`. This document describes how types flow through the system and the key components involved.

## Type System Boundaries

Toasty has two distinct type systems with different responsibilities:

### 1. Rust-Level Type System (Compile-Time Safety)

At the Rust level, each model is a distinct type:

```rust
#[derive(Model)]
struct User {
    #[key]
    #[auto]
    id: u64,
    name: String,
    email: String,
}

#[derive(Model)]
struct Todo {
    #[key]
    #[auto]
    id: u64,
    user_id: u64,
    title: String,
}

// Toasty generates type-safe field access preventing type mismatches:
User::get_by_email(&db, "john@example.com").await?;  // ✓ String matches email field
User::filter_by_id(&user_id).filter(User::FIELDS.name().eq("John")).all(&db).await?;  // ✓ String matches name field

// Type system prevents field/model confusion:
// User::FIELDS.title()  // ← Compile error! User has no title field
// Todo::FIELDS.email()  // ← Compile error! Todo has no email field
// User::FIELDS.name().eq(&todo_id)  // ← Compile error! u64 doesn't match String
```

The query builder API maintains this type safety through generics and traits, preventing you from accidentally mixing model types or referencing non-existent fields. The API uses generic types (`Statement<M>`, `Select<M>`, etc.) that wrap `toasty_core::stmt` types.

### 2. Query Engine Type System (Runtime)

When `db.exec(statement)` is called, the generic `<M>` parameter is erased:

```rust
// Generated query builder returns a typed wrapper
let query: FindUserById = User::find_by_id(&id);

// .into() converts to Statement<User>
let statement: Statement<User> = query.into();

// At db.exec() - generic is erased, .untyped is extracted
pub async fn exec<M: Model>(&self, statement: Statement<M>) -> Result<ValueStream> {
    engine::exec(self, statement.untyped).await  // <- Only toasty_core::stmt::Statement
}
```

At this boundary, the statement becomes untyped (no Rust generic), but the engine tracks the type of value the statement evaluates to using `stmt::Type`. Initially, this remains at the model-level—a query for `User` evaluates to `Type::List(Type::Model(user_model_id))`. During lowering, these convert to structural record types for database execution.

## Type Flow Through the System

```
Rust API → Query Builder → Engine Entry → Lowering/Planning → Execution
    ↓           ↓              ↓               ↓                  ↓
Distinct    Type-Safe      Type::Model     Type::Record       stmt::Value
Types       Generics       (no generics)                      (typed)
(compile)   (compile)      (runtime)       (runtime)          (runtime)
```

At lowering, statements that evaluate to `Type::Model(model_id)` are converted to evaluate to `Type::Record([field_types...])`. This conversion enables the engine to work with concrete field types for database operations.

## Detailed Architecture

### Query Engine Entry Point

When the engine receives a `toasty_core::stmt::Statement`, it processes through verification, lowering, planning, and execution:

```rust
pub(crate) async fn exec(&self, stmt: Statement) -> Result<ValueStream> {
    if cfg!(debug_assertions) {
        self.verify(&stmt);
    }

    // Lower the statement to High-level intermediate representation
    let hir = self.lower_stmt(stmt)?;

    // Translate into a series of driver operations
    let plan = self.plan_hir_statement(hir)?;

    // Execute the plan
    self.exec_plan(plan).await
}
```

### Lowering Phase (Model-to-Table Transformation)

The lowering phase transforms statements from model-level to table-level representations.

**Example 1: Simple query**

```rust
// Before lowering (toasty_core::stmt::Statement)
SELECT MODEL FROM User WHERE id = ?
// Evaluates to: Type::List(Type::Model(user_model_id))
// Note: At model-level, no specific fields are selected

// After lowering
SELECT id, name, email FROM users WHERE id = ?
// Evaluates to: Type::List(Type::Record([Type::Id, Type::String, Type::String]))
```

**Example 2: Query with association**

```rust
// Before lowering (toasty_core::stmt::Statement)
SELECT MODEL FROM User INCLUDE todos WHERE id = ?
// Evaluates to: Type::List(Type::Model(user_model_id))
// where todos field is Type::List(Type::Model(todo_model_id))

// After lowering
SELECT id, name, email, (
    SELECT id, title, user_id FROM todos WHERE todos.user_id = users.id
) FROM users WHERE id = ?
// Evaluates to: Type::List(Type::Record([
//   Type::Id, Type::String, Type::String,
//   Type::List(Type::Record([Type::Id, Type::String, Type::Id]))
// ]))
```

### Planning and Variable Types

During planning, the engine assigns variables to hold intermediate results (see [Query Engine Architecture](query-engine.md) for details on the execution model). Each variable is registered with its type, which is always `Type::List(...)` or `Type::Unit`.

### Execution

At execution time, the `VarStore` holds the type information from planning. When storing a value stream in a variable, the store associates the expected type with it. The value stream ensures each value it yields conforms to that type. This type information carries through to the final result returned to the user.

### Type Inference

While statements entering the engine have known types, planning constructs new expressions—projections, filters, and merge qualifications—whose types aren't explicitly declared. The engine must infer these types from the expression structure to register variables correctly.

Type inference is handled by `ExprContext`, which walks expression trees and determines their result types based on the schema. For example, a column reference's type comes from the schema definition, and a record expression's type is built from its field types.

```rust
// Create context for type inference
let cx = stmt::ExprContext::new_with_target(&*self.engine.schema, stmt);

// Infer type of an expression reference
let ty = cx.infer_expr_reference_ty(expr_reference);

// Infer type of a full expression with argument types
let ret = ExprContext::new_free().infer_expr_ty(expr.as_expr(), &args);
```
