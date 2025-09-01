# Toasty Type System Architecture

## Overview

Toasty connects compile-time Rust type safety with runtime query execution through a type system. Inside the query engine, the type system ensures correctness and catches bugs during statement processing, planning, and validation phases, though it is not required for runtime execution. This document describes the current architecture of Toasty's type system, how types flow through the system, and the key components involved.

## Type System Boundaries

Toasty has two distinct type systems with different responsibilities:

### 1. Rust-Level Type System (Compile-Time Safety)

At the Rust level, models are **real, distinct types** with compile-time guarantees:

```rust
#[derive(Model)]
struct User {
    #[key] #[auto] id: Id<Self>,
    name: String,
    email: String,
}

#[derive(Model)]
struct Todo {
    #[key] #[auto] id: Id<Self>,
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

The query builder API maintains this type safety through generics and traits, preventing you from accidentally mixing model types or referencing non-existent fields. The API uses generic wrapper types (`Statement<M>`, `Select<M>`, etc.) that wrap `toasty_core::stmt` types to provide compile-time safety while erasing to core types at runtime.

### 2. Query Engine Type System (Runtime Model-level)

When statements enter the query engine, models retain their model-level types:

- `User` → `Type::Model(user_model_id)`
- `Todo` → `Type::Model(todo_model_id)`
- Associations → `Type::Model(target_model_id)` or `Type::List(Type::Model(target_model_id))`

Models maintain distinct identities through their `ModelId` and preserve model-level information about relationships. The conversion to structural `Type::Record([...])` happens during the lowering phase, not at engine entry.

## Type Flow Through the System

The current query execution follows this pipeline:

```
Rust API → Query Builder → Engine Entry → Model Planning → Lowering → Execution
    ↓           ↓              ↓              ↓              ↓           ↓
Distinct    Type-Safe      Type::Model    Associations   Type::Record  Runtime
Types       Generics       (no generics)  Subqueries     Structural    Values
(compile)   (compile)      (runtime)      (runtime)      (runtime)     (runtime)
```

**The Generic Erasure Boundary**: When `db.exec(statement)` is called, the generic `<M>` parameter is discarded:

```rust
// User code - generic wrapper with compile-time safety
let statement: Statement<User> = User::filter_by_id(&id).into();

// At db.exec() - generic is stripped, .untyped is extracted
pub async fn exec<M: Model>(&self, statement: Statement<M>) -> Result<ValueStream> {
    engine::exec(self, statement.untyped).await  // <- Only toasty_core::stmt::Statement
}
```

At this boundary, we transition from compile-time generic safety to runtime model-level types using `Type::Model(model_id)`. Model-level planning (associations, subqueries) happens before lowering converts these to structural `Type::Record` types.

## Detailed Architecture

### Query Engine Entry Point (Model-Level Types)

When the engine receives a `toasty_core::stmt::Statement` (after generic erasure), models retain model-level type information:

```rust
// Query builder API (type-safe) - produces Statement<User>
User::filter_by_id(&id).include(User::FIELDS.todos())
    ↓
// After generic erasure at db.exec() - becomes toasty_core::stmt::Statement
// User is Type::Model(User::id())
// Associations are Type::Model(target_id) or Type::List(Type::Model(target_id))
// ModelId preserves model-level information about relationships
```

At this point, generics have been erased and we have model-level types that preserve relationship information through ModelId references. Initial verification and simplification work happens at this model level before planning begins.

### Model-Level Planning Phase

Before lowering occurs, the planner performs model-level operations using `Type::Model` references:

**Association Resolution**: Determines how to handle relationships between models:
- `BelongsTo` fields remain as `Type::Model(target_model_id)` references
- `HasMany` fields remain as `Type::List(Type::Model(target_model_id))` references
- Planning decisions are made about which associations to include

**Subquery Handling**: Processes nested queries while maintaining model-level type information throughout the query tree.

**Include Processing**: Analyzes which associations need to be loaded and plans the execution strategy, all while working with `Type::Model` references that preserve relationship semantics.

### Lowering Step (Model-to-Table Type Transformation)

The `lower_stmt_query()` function marks the boundary between model-level and table-level operations, transforming statements for database execution:

**Type-Level Transformation:**
- **Before lowering**: `Type::Model(model_id)` → **After lowering**: `Type::Record([primitive_types...])`
- **Association types**: `Type::List(Type::Model(target_id))` becomes placeholders in the record structure
- **Value-Level**: Values remain as `Value::Record(...)` regardless of type representation

**Expression Lowering:**
```rust
// Before lowering
Returning::Star  // Semantic: "return the full User model"

// After lowering
Returning::Expr(table_to_model)  // Structural: "return [id, name, null, null]"
```

**Static Mapping Construction:**
The `table_to_model` mapping is pre-built during schema construction:
```rust
// In schema/builder/table.rs
app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
    self.table_to_model.push(stmt::Value::Null.into()); // Association fields become Null
}
```

**Characteristics:**
- *$$*Type flattening**: `Type::Model(id)` becomes `Type::Record([field_types...])`
- **Association erasure**: Relationship fields become `Null` values in the structural record
- **Context independent**: The mapping doesn't vary based on query context
- **One-way transform**: Optimized for database execution, not reversibility
- **Planning boundary**: Separates model-level planning from table-level execution

### Table-Level Execution

After lowering, all operations work with structural `Type::Record` types:

**Type Inference**: The `partition_returning()` function infers types from lowered expressions:
```rust
pub(crate) fn partition_returning(&self, stmt: &mut stmt::Returning) -> eval::Func {
    let ret = self.infer_expr_ty(stmt.as_expr(), &[]);  // Works with Type::Record
    // ... rest of function
}
```

**Variable Tracking**: The `VarTable` maintains type information for execution:
```rust
pub(crate) struct VarTable {
    vars: Vec<stmt::Type>,  // All Type::Record or primitives after lowering
}
```

**Include Reconstruction**: When includes are present, the runtime reconstructs associations by expanding `Null` placeholders:
```rust
// In planner/select.rs - Replace Null with actual types for includes
app::FieldTy::HasMany(rel) => {
    let target_record_type = self.infer_model_record_type(target_model);
    fields[*field_idx] = stmt::Type::list(target_record_type);
}
```

**Runtime Association Loading**: Execution fills in association data:
```rust
// Runtime association loading
source_item[action.field.index] = stmt::ValueRecord::from_vec(associated).into();
```

All execution operations work exclusively with structural types, never with `Type::Model` references.

## Key Components

### stmt::Type Hierarchy

```rust
pub enum Type {
    String,
    I64,
    Bool,
    Null,
    Record(Vec<Type>),
    List(Box<Type>),
    Model(ModelId),
    // ... other variants
}
```

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
    pub table_to_model: stmt::ExprRecord,
}
```

#### Type Expansion During Include Processing

When associations are included in queries, the planner expands types from the lowered `Null` placeholders:

**The Challenge**: Model associations can be cyclic (A → B → A → ...). Direct expansion would create infinite recursion.

**The Solution**: Use semantic model types and expand only when included:

```rust
// Base model type uses semantic references
User fields: [Id, String, List(Type::Model(TodoModelId))]
Todo fields: [Id, String, Type::Model(UserModelId)]

// During lowering, associations become Null in table_to_model
User lowered: [id, name, null]  // Association field becomes Null

// When includes are present, planner expands specific associations
User with todos included: [Id, String, List(Record([Id, String, Type::Model(UserModelId)]))]
```

**Implementation**: The `infer_model_record_type()` function constructs full structural types when needed:

```rust
// In planner/select.rs - Include processing
app::FieldTy::HasMany(rel) => {
    let target_record_type = self.infer_model_record_type(target_model);
    fields[*field_idx] = stmt::Type::list(target_record_type);
}
```

This approach avoids infinite recursion while allowing precise typing of included relationships.

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
- **Include expansion**: Type system handles cyclic associations through selective expansion
- **Two-phase typing**: Compile-time nominal types, runtime model-level then structural types
- **Schema-driven**: Types derived from schema definitions
- **Inference-based**: Runtime types determined through expression analysis
- **Static mappings**: Pre-computed transformations for execution efficiency

## Integration Points

The type system connects with other Toasty subsystems:

- **Codegen**: Generates schema definitions with type information
- **Query Builder**: Creates type-safe query construction APIs
- **Database Drivers**: Converts types to database-specific representations
- **Execution Engine**: Uses type information for validation and optimization

This architecture provides compile-time safety and runtime correctness while maintaining efficient query execution across different database backends.