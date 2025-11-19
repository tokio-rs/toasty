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
    id: Id<Self>,
    name: String,
    email: String,
}

#[derive(Model)]
struct Todo {
    #[key]
    #[auto]
    id: Id<Self>,
    user_id: Id<User>,
    title: String,
}

// Toasty generates type-safe field access preventing type mismatches:
User::get_by_email(&db, "john@example.com").await?;  // ✓ String matches email field
User::filter_by_id(&user_id).filter(User::FIELDS.name().eq("John")).all(&db).await?;  // ✓ String matches name field

// Type system prevents field/model confusion:
// User::FIELDS.title()  // ← Compile error! User has no title field
// Todo::FIELDS.email()  // ← Compile error! Todo has no email field
// User::FIELDS.name().eq(&todo_id)  // ← Compile error! Id<Todo> doesn't match String
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

### Type Inference

Type inference is handled by `ExprContext` which provides methods to infer the type a given expression evaluates to:

```rust
// Create context for type inference
let cx = stmt::ExprContext::new_with_target(&*self.engine.schema, stmt);

// Infer type of an expression reference
let ty = cx.infer_expr_reference_ty(expr_reference);

// Infer type of a full expression with argument types
let ret = ExprContext::new_free().infer_expr_ty(expr.as_expr(), &args);
```

## Key Components

### stmt::Type Hierarchy

The `stmt::Type` enum represents all types in the query engine (see `toasty-core/src/stmt/ty.rs` for complete definition). Key variants include:

- Primitives: `Bool`, `String`, `I8`-`I64`, `U8`-`U64`, `Uuid`, `Bytes`
- Model types: `Id(ModelId)`, `Key(ModelId)`, `Model(ModelId)`, `ForeignKey(FieldId)`
- Compound: `List(Box<Type>)`, `Record(Vec<Type>)`, `Enum(TypeEnum)`
- Special: `Null`, `Unit`, `Unknown`, `SparseRecord(PathFieldSet)`

### Schema Mapping

Each model has mapping information stored in `schema::mapping::Model`:

```rust
pub struct Model {
    pub id: ModelId,
    pub table: TableId,
    pub columns: Vec<ColumnId>,
    pub fields: Vec<Option<Field>>,
    pub model_to_table: stmt::ExprRecord,
    pub model_pk_to_table: stmt::Expr,
    pub table_to_model: TableToModel,
}
```

### Field Expression Types

Association fields store their expression types:

```rust
// BelongsTo
pub struct BelongsTo {
    pub target: ModelId,
    pub expr_ty: stmt::Type,  // Type::Model(target_model_id)
    // ...
}

// HasMany
pub struct HasMany {
    pub target: ModelId,
    pub expr_ty: stmt::Type,  // Type::List(Box::new(Type::Model(target_model_id)))
    // ...
}
```

## Type System Characteristics

- **Compile-time safety**: Rust type system prevents model confusion
- **Model-level preservation**: `Type::Model` maintains relationship information until lowering
- **Database optimization**: Lowering converts to `Type::Record` for efficient execution
- **Typed evaluation**: `eval::Func` captures argument and return types for runtime execution
- **Schema-driven**: Types derived from schema definitions
- **Inference-based**: Runtime types determined through `ExprContext` analysis

## Integration Points

The type system connects with other Toasty subsystems:

- **Codegen**: Generates schema definitions with type information
- **Query Builder**: Creates type-safe query construction APIs
- **Database Drivers**: Converts types to database-specific representations
- **Execution Engine**: Uses type information for validation and value typing

This architecture provides compile-time safety and runtime correctness while maintaining efficient query execution across different database backends.
