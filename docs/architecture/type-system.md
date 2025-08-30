# Toasty Type System Architecture

## Overview

Toasty employs a sophisticated type system that bridges compile-time Rust type safety with runtime query execution. This document describes the current architecture of Toasty's type system, how types flow through the system, and the key components involved.

## Type System Boundaries

Toasty has two distinct type systems with different responsibilities:

### 1. Rust-Level Type System (Compile-Time Safety)

At the Rust level, models are **real, distinct types** with compile-time guarantees:

```rust
#[derive(Model)]
struct User { id: Id<User>, name: String, ... }

#[derive(Model)]  
struct Post { id: Id<Post>, title: String, ... }

// Even if User and Post had identical field types, they remain distinct:
fn process_user(user: User) { ... }
fn process_post(post: Post) { ... }

// Rust prevents mixing them:
// process_user(post);  // ← Compile error!
```

The query builder API maintains this type safety through generics and traits, preventing you from accidentally mixing model types or referencing non-existent fields.

### 2. Query Engine Type System (Runtime Structural)

When statements enter the query engine, models become **structural types**:

- `User` → `Type::Model(user_model_id)` → effectively `Type::Record([Id, String, ...])`
- `Post` → `Type::Model(post_model_id)` → effectively `Type::Record([Id, String, ...])`

Even if two models have identical field structures, they maintain distinct identities through their `ModelId`. However, the query engine works with the structural representation.

## Type Flow Through the System

The current query execution follows this pipeline:

```
Rust API → Query Builder → Query Engine → Schema Planning → Lowering → Execution
    ↓           ↓              ↓              ↓               ↓           ↓
Distinct    Type-Safe      Model Types    Rich Types      Primitive   Runtime  
Types       Generics       (structural)   (from schema)   Types       Values
(compile)   (compile)      (runtime)      (runtime)       (runtime)   (runtime)
```

**The Boundary**: When the query builder creates a `stmt::Statement`, we transition from Rust's nominal type system to the query engine's structural type system.

## Detailed Architecture

### Query Engine Entry Point (Structural Types)

When the query builder creates a `stmt::Statement`, models become structural:

```rust
// Query builder API (type-safe)
User::filter_by_id(&id).include(User::FIELDS.posts())
    ↓
// Creates stmt::Statement where User is Type::Model(user_model_id)
// No runtime type info stored in statement - just references to ModelId
```

At this point, we've crossed the boundary from nominal types (distinct Rust types) to structural types (records identified by ModelId).

### Schema-Aware Planning (Rich Semantic Types)

When query planning begins, the planner has access to rich semantic types from the schema:

- **Model Fields**: Have `expr_ty()` that returns semantic types:
  - `BelongsTo` → `Type::Model(target_model_id)`
  - `HasMany` → `Type::List(Type::Model(target_model_id))`
  - Primitives → `Type::String`, `Type::Id`, etc.

- **Schema Access**: The planner can query `field.expr_ty()` to get rich type information
- **Context Awareness**: Knows about includes, model relationships, and field semantics

### Lowering Step (Statement Transformation)

The `lower_stmt_query()` function transforms statements for database execution:

**Before lowering:**
```rust
Returning::Star  // Semantic: "return the full User model"
```

**After lowering:**
```rust  
Returning::Expr(table_to_model)  // Concrete: "return [id, name, null, null]"
```

**Key Characteristics:**
- **Static Mapping**: `table_to_model` is pre-built during schema construction:
  ```rust
  // In schema/builder/table.rs
  app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
      self.table_to_model.push(stmt::Value::Null.into()); // Always Null!
  }
  ```
- **Context Independent**: The mapping doesn't vary based on query context
- **One-Way Transform**: Optimized for database execution, not reversibility

### Type Inference System

The `partition_returning()` function infers types from expressions:

```rust
pub(crate) fn partition_returning(&self, stmt: &mut stmt::Returning) -> eval::Func {
    let ret = self.infer_expr_ty(stmt.as_expr(), &[]);
    // ... rest of function
}
```

**Type Inference Features:**
- Analyzes expression structure to determine result types
- Works with lowered expressions (post-transformation)
- Handles primitive types and record structures

### Variable Type Tracking

Variables in the `VarTable` track types throughout execution:

```rust
pub(crate) struct VarTable {
    vars: Vec<stmt::Type>,
}
```

Each variable slot maintains its type information for validation and execution planning.

### Runtime Execution

During execution, `Associate` actions can modify data structures:

```rust
// Runtime association loading
source_item[action.field.index] = stmt::ValueRecord::from_vec(associated).into();
```

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
    pub record_ty: stmt::Type,  // Cached structural type (associations as semantic types)
}
```

#### The Type Erasure and Cyclic Association Problem

The `record_ty` field solves a fundamental challenge in Toasty's type system:

**Problem**: Model associations can be cyclic (A → B → A → ...). If we always expanded association types to their full record structures, we would create infinite recursion:

```rust
// This would cause infinite recursion:
User = Record([Id, String, List(Post)])  // User has many Posts
Post = Record([Id, String, User])        // Post belongs to User
User = Record([Id, String, List(Record([Id, String, User]))])  // Expanding Post
User = Record([Id, String, List(Record([Id, String, Record([Id, String, List(Post)])]))]) // ∞
```

**Solution**: Use **type erasure** and **lazy expansion**:

1. **Base Record Type**: Store associations as semantic types (`Type::Model(ModelId)`, `Type::List(Type::Model(ModelId))`) to avoid infinite recursion
2. **Query-Time Expansion**: Only expand associations to their full record types when actually used in queries with includes
3. **Cached Performance**: Pre-compute the base type once during schema building

```rust
// Safe, finite representation:
User.record_ty = Record([Id, String, List(Model(PostModelId))])
Post.record_ty = Record([Id, String, Model(UserModelId)])

// Expanded only when needed:
User.query_with_posts_included = Record([Id, String, List(Record([Id, String, Model(UserModelId)]))])
```

#### Implementation Details

The type expansion logic is implemented in the query planner:

```rust
// In planner.rs
fn build_include_aware_model_type(&self, model: &app::Model, includes: &[stmt::Path]) -> stmt::Type {
    // Start with cached record_ty (associations as semantic types)
    let mut record_ty = model_mapping.record_ty.clone();
    
    // Only expand included associations
    for include in includes {
        let field = &model.fields[field_idx];
        match &field.ty {
            app::FieldTy::HasMany(has_many) => {
                // Replace List(Model(ModelId)) with List(actual_record_type)
                let target_record_type = target_mapping.record_ty.clone();
                field_types[field_idx] = stmt::Type::list(target_record_type);
            }
            // Similar for BelongsTo and HasOne...
        }
    }
}
```

This approach ensures:
- **Finite types**: Base record types never recurse infinitely
- **Lazy expansion**: Only compute full types when needed
- **Query accuracy**: Type checking matches runtime data structure

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

### Strengths

- **Compile-time safety**: Rust type system prevents model confusion
- **Rich semantic information**: Schema provides detailed type relationships
- **Efficient execution**: Lowering optimizes for database performance
- **Flexible inference**: Can determine types from expressions

### Current Architecture Patterns

- **Two-phase typing**: Compile-time nominal, runtime structural
- **Schema-driven**: Types derived from schema definitions
- **Inference-based**: Runtime types determined through analysis
- **Static mappings**: Pre-computed transformations for efficiency

## Integration Points

The type system integrates with several other Toasty subsystems:

- **Codegen**: Generates schema definitions with embedded type information
- **Query Builder**: Provides type-safe query construction APIs
- **Database Drivers**: Converts types to database-specific representations
- **Execution Engine**: Uses type information for validation and optimization

This architecture enables Toasty to provide both compile-time safety and runtime flexibility while maintaining efficient query execution across different database backends.