# Type System Design: Maintaining Type Information Throughout Query Execution

## Overview

The current Toasty query engine suffers from a fundamental architectural issue: type information is lost during the transformation from application-level queries to table-level queries. This document analyzes the current state, identifies the core problems, and proposes a solution for maintaining complete type information throughout the entire query planning and execution pipeline.

## Problem Statement

The recent work on fixing the `.include()` feature exposed a critical flaw in the type system architecture. While single includes worked, multiple includes failed due to type mismatches during query execution. The root cause is not the include feature itself, but rather a systematic loss of type information that makes it impossible to correctly track query transformations.

### Key Issues

1. **Type Information Loss**: The lowering step from app-level to table-level throws away semantic type information
2. **Static Mappings**: Pre-built `table_to_model` mappings don't account for query-specific context (like includes)
3. **Disconnected Type Systems**: App-level and table-level type systems don't communicate
4. **Runtime Type Changes**: Actions like `Associate` modify data structures without updating type information

## Current Architecture: Status Quo

### Type System Boundaries

Toasty has two distinct type systems with different responsibilities:

#### 1. Rust-Level Type System (Compile-Time Safety)
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

#### 2. Query Engine Type System (Runtime Structural)
When statements enter the query engine, models become **structural types**:

- `User` → `Type::Model(user_model_id)` → effectively `Type::Record([Id, String, ...])`
- `Post` → `Type::Model(post_model_id)` → effectively `Type::Record([Id, String, ...])`

Even if two models have identical field structures, they maintain distinct identities through their `ModelId`. However, the query engine works with the structural representation.

### Type Flow Through the System

The current query execution follows this pipeline:

```
Rust API → Query Builder → Query Engine → Schema Planning → Lowering → Execution
    ↓           ↓              ↓              ↓               ↓           ↓
Distinct    Type-Safe      Model Types    Rich Types      Primitive   Runtime  
Types       Generics       (structural)   (from schema)   Types       Values
(compile)   (compile)      (runtime)      (runtime)       (runtime)   (runtime)
```

**The Boundary**: When the query builder creates a `stmt::Statement`, we transition from Rust's nominal type system to the query engine's structural type system.

### 3. Query Engine Entry Point (Structural Types)

When the query builder creates a `stmt::Statement`, models become structural:

```rust
// Query builder API (type-safe)
User::filter_by_id(&id).include(User::FIELDS.posts())
    ↓
// Creates stmt::Statement where User is Type::Model(user_model_id)
// No runtime type info stored in statement - just references to ModelId
```

At this point, we've crossed the boundary from nominal types (distinct Rust types) to structural types (records identified by ModelId).

### 4. Schema-Aware Planning (Rich Semantic Types)

When query planning begins, the planner has access to rich semantic types from the schema:

- **Model Fields**: Have `expr_ty()` that returns semantic types:
  - `BelongsTo` → `Type::Model(target_model_id)`
  - `HasMany` → `Type::List(Type::Model(target_model_id))`
  - Primitives → `Type::String`, `Type::Id`, etc.

- **Schema Access**: The planner can query `field.expr_ty()` to get rich type information
- **Context Awareness**: Knows about includes, model relationships, and field semantics

### 5. Lowering Step (Type Information Destruction)

The `lower_stmt_query()` function performs a **lossy transformation**:

**Before lowering:**
```rust
Returning::Star  // Semantic: "return the full User model"
```

**After lowering:**
```rust  
Returning::Expr(table_to_model)  // Primitive: "return [id, name, null, null]"
```

**Key Issues:**

- **Static Mapping**: `table_to_model` is pre-built during schema construction:
  ```rust
  // In schema/builder/table.rs
  app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
      self.table_to_model.push(stmt::Value::Null.into()); // Always Null!
  }
  ```

- **Context Unaware**: The mapping doesn't know about includes, filters, or other query context
- **One-Way Transform**: Cannot reconstruct semantic meaning from lowered form

### 6. Type Inference in partition_returning()

The `partition_returning()` function infers types from the already-lowered expression:

```rust
pub(crate) fn partition_returning(&self, stmt: &mut stmt::Returning) -> eval::Func {
    let ret = self.infer_expr_ty(stmt.as_expr(), &[]); // ← Uses lowered expression!
    // ... rest of function
}
```

**Problems:**
- Sees `Value::Null` for all association fields
- Cannot distinguish between "unloaded association" and "actually null"
- Type inference happens **after** semantic information is destroyed

### 7. Variable Type Tracking

Variables in the `VarTable` track types, but only primitive types:

```rust
pub(crate) struct VarTable {
    vars: Vec<stmt::Type>, // ← Only primitive types survive lowering
}
```

### 8. Runtime Execution Issues

During execution, `Associate` actions modify data structures:

```rust
// OLD CODE: Wrong data structure for HasMany
source_item[action.field.index] = stmt::ValueRecord::from_vec(associated).into();
```

**Problems:**
- Runtime modifications aren't reflected in type system
- Type checking uses stale type information
- Second include fails because it sees modified data with original types

### The Include Feature Failure

The multiple includes failure illustrates the type system breakdown:

1. **First Include Planning**: Works with original types `[Id, String, Null, Null]`
2. **First Associate Execution**: Modifies data to `[Id, String, Record([posts]), Null]`
3. **Second Include Planning**: Uses modified data as input with stale types
4. **Type Check Failure**: `Value::Record([posts])` doesn't match expected `Type::Null`

## Root Cause Analysis

The fundamental issue is **architectural**: we have two disconnected type systems:

### Model-Level Type System (Rich)
- Semantic meaning preserved
- Knows about relationships and models
- Context-aware (could account for includes)

### Table-Level Type System (Primitive)
- Only structural information
- No semantic meaning
- Context-unaware

**The lowering step is a one-way, lossy transformation** that destroys information needed for correct query execution.

## Proposed Solution: `Typed<T>` Wrapper

### Design Principles

1. **Preserve Semantic Information**: Don't lose model-level type knowledge
2. **Context Awareness**: Type system should know about query context (includes, filters, etc.)
3. **Track Transformations**: Actions that modify data should update types accordingly
4. **Simple Implementation**: Avoid complex parallel type trees
5. **Clean Separation**: Type information travels with statements

### Core Design

The solution is to use a `Typed<T>` wrapper that carries type information alongside statements:

```rust
#[derive(Debug, Clone)]
struct Typed<T> {
    /// The statement/expression
    value: T,
    
    /// The type this evaluates to
    ty: stmt::Type,
}
```

**Key Insight**: In Toasty's type system, `Type::Model(model_id)` is essentially an alias for a specific `Type::Record` structure. Models ARE records - they're just records with semantic meaning attached.

**Association Type Preservation Example:**

Consider these models:
```rust
#[derive(Model)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    #[has_many]
    todos: toasty::HasMany<Todo>,
}

#[derive(Model)]
struct Todo {
    #[key]
    #[auto]
    id: Id<Self>,

    #[index]
    user_id: Id<User>,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,
}
```

When a User query calls `.include(User::FIELDS.todos())`, the resulting `Typed<Statement>` for that query will have a type of:

```
Record[String, List[Record[String, String, Null]]]
```

Where:
- The root record represents a User
- First field is the `id`
- Second field is the `todos` association (included, so it's a List of Todo records)
- The Todo record type has Null as its third field because the `user` association is not included

**How It Works:**

1. **Application-Level Statement**:
   ```rust
   // The query builds a Typed<Statement> with the correct association type
   let typed_stmt = Typed {
       value: stmt,
       ty: Type::Record([Type::String, Type::List(Type::Record([Type::String, Type::String, Type::Null]))]),
   };
   ```

2. **Lowering Preserves Type**:
   ```rust
   fn lower_stmt_query(typed_stmt: &mut Typed<stmt::Statement>) {
       // Lowering transforms the statement but the type stays the same
       // Association type information is preserved throughout
       LowerStatement::from_model(schema, model)
           .visit_stmt_query_mut(&mut typed_stmt.value);
   }
   ```

### Entry Point: Query Builder to Engine

The `Typed<Statement>` is created at the boundary between the query builder and the query engine:

```rust
// Query builder creates stmt::Statement (no type info yet)
User::filter_by_id(&id).include(User::FIELDS.todos())
    ↓
// Produces: stmt::Query with includes tracked separately

// Engine entry point: plan_stmt_query() 
impl Engine {
    pub fn plan_stmt_query(&mut self, stmt: stmt::Query) -> Result<plan::VarId> {
        // This is where Typed<Statement> is first created
        let typed_stmt = self.planner.build_typed_statement(stmt)?;
        
        // Now we have rich type information for the rest of the pipeline
        self.planner.plan_typed_select(typed_stmt)
    }
}
```

**Key Insight**: The transition happens when the `stmt::Statement` (which has no embedded type information) enters the query engine and gets wrapped in `Typed<Statement>` with computed type information.

### Type Computation Algorithm

The type is computed by leveraging existing infrastructure and modifying it based on includes from the query.

**Existing Infrastructure:**
- Each model already has a `table_to_model` expression (built during schema construction)
- Association fields already store their correct types in `field.expr_ty()`:
  - `BelongsTo` → `Type::Model(target_model_id)`  
  - `HasMany` → `Type::List(Box::new(Type::Model(target_model_id)))`

**The Problem:** The current `table_to_model` is static - it always sets association fields to `Null`

**The Solution:** Add a `record_ty` field to cache the record type, then modify based on includes:

First, extend `schema::mapping::Model` to cache the record type:
```rust
#[derive(Debug, Clone)]
pub struct Model {
    // ... existing fields ...
    
    /// How to map a table record to a model record
    pub table_to_model: stmt::ExprRecord,
    
    /// The record type for this model (all primitive types + associations as Null)
    pub record_ty: stmt::Type,
}
```

Then the type computation becomes much simpler:

```rust
impl Planner<'_> {
    fn build_typed_statement(&mut self, stmt: stmt::Query) -> Result<Typed<stmt::Query>> {
        let source_model = stmt.body.as_select().source.as_model();
        let model = self.schema.app.model(source_model.model);
        let includes = extract_includes_from_query(&stmt); // Extract from query AST
        
        // Start with cached record type and modify for includes
        let ty = self.build_include_aware_model_type(model, &includes);
        
        Ok(Typed { value: stmt, ty })
    }
    
    fn build_include_aware_model_type(&self, model: &Model, includes: &[Path]) -> stmt::Type {
        // Start with the cached record type (primitive types + associations as Null)  
        let model_mapping = self.schema.mapping.model(model.id);
        let mut record_ty = model_mapping.record_ty.clone();
        
        // Modify association fields based on includes
        if let stmt::Type::Record(ref mut field_types) = record_ty {
            for field in &model.fields {
                match &field.ty {
                    FieldTy::HasMany(_) | FieldTy::BelongsTo(_) | FieldTy::HasOne(_) => {
                        if includes.contains(&field.path()) {
                            // Replace Null with the actual association type from field.expr_ty()
                            field_types[field.id.index] = field.expr_ty().clone();
                        }
                        // If not included, stays as Null (already set during schema construction)
                    }
                    _ => {
                        // Primitive fields already correct in the cached type
                    }
                }
            }
        }
        
        record_ty
    }
}
```

**Algorithm Summary**:
1. **Extract includes** from the query AST  
2. **Start with cached `record_ty`** (primitive fields already correct, associations as Null)
3. **Modify association fields**: For included associations, replace Null with actual type from `field.expr_ty()`
4. **Return modified type directly** - no inference needed!

**Key Benefits:**
- **More efficient** - no type inference required, work directly with types
- **Cleaner separation** - types cached separately from expressions  
- **Simpler implementation** - just clone and modify the cached type
- **Leverages field.expr_ty()** - association types already computed during schema construction

3. **partition_returning Gets Correct Type**:
   ```rust
   fn partition_returning(typed_returning: &mut Typed<stmt::Returning>) -> eval::Func {
       // No need to infer - we already have the correct type!
       let ret_type = typed_returning.ty.clone();
       
       // Partition the expression, update if needed
       match partition_expr(&mut typed_returning.value) {
           // ... partitioning logic
       }
       
       eval::Func::from_stmt_unchecked(expr, args, ret_type)
   }
   ```

**Integration Pattern**:
```rust
impl Planner<'_> {
    pub(super) fn plan_stmt_select(
        &mut self,
        cx: &Context,
        stmt: stmt::Query,
    ) -> Result<plan::VarId> {
        // Step 1: Create Typed<Statement> with computed type
        // - Extracts includes from query AST 
        // - Computes Record type based on model + includes
        // - Creates Typed wrapper with correct association types
        let mut typed_stmt = self.build_typed_statement(stmt)?;
        
        // Step 2: Lowering preserves the computed type
        let source_model = typed_stmt.value.body.as_select().source.as_model().clone(); 
        let model = self.schema.app.model(source_model.model);
        self.lower_typed_stmt_query(model, &mut typed_stmt);
        
        // Step 3: partition_returning uses the pre-computed type (no inference needed)
        let project = self.partition_typed_returning(
            &mut typed_stmt.value.body.as_select_mut().returning,
            &typed_stmt.ty
        );
        
        // Step 4: Register variable with the correct type
        let output = self.var_table.register_var(typed_stmt.ty.clone());
        
        // ... rest of planning
    }
    
    /// Creates Typed<Statement> using cached record_ty modified for includes
    fn build_typed_statement(&mut self, stmt: stmt::Query) -> Result<Typed<stmt::Query>> {
        let source_model = stmt.body.as_select().source.as_model();
        let model = self.schema.app.model(source_model.model);
        
        // Extract includes from the query (from WHERE, JOIN, or dedicated include clauses)
        let includes = self.extract_includes_from_query(&stmt);
        
        // Clone cached record type and modify association fields for includes
        let ty = self.build_include_aware_model_type(model, &includes);
        
        Ok(Typed { value: stmt, ty })
    }
}
```

**Advantages:**
- **Generalizable**: Works for all statement types, not just selects
- **Clean Separation**: Type information travels with statements
- **No API Changes**: Existing code continues to work via `.value` access
- **Simple**: Single wrapper, no complex threading
- **Correct by Construction**: Types are built with query context from the start

**Challenges:**
- **Wrapper Proliferation**: Many functions would need `Typed<T>` parameters
- **Type Updates**: Need to ensure type stays in sync when statements change
- **Migration**: Existing code needs updates to use `Typed<T>`

**Type Update Pattern**:
```rust
impl<T> Typed<T> {
    /// Update the type when the statement changes
    fn update_type(&mut self, new_type: stmt::Type) {
        self.ty = new_type;
    }
    
    /// Helper to transform both value and type together
    fn transform<F>(&mut self, f: F) 
    where 
        F: FnOnce(&mut T, &mut stmt::Type)
    {
        f(&mut self.value, &mut self.ty);
    }
}
```

### Benefits

The `Typed<T>` wrapper approach provides:

1. **Simpler Architecture**: Single wrapper carries type information with statements
2. **More Generalizable**: Works for all statement types, not query-specific
3. **Association Type Preservation**: Types reflect included associations throughout lowering
4. **Clean Implementation**: No need for separate context tracking or complex threading

## Implementation Plan

### Phase 1: Foundation (Week 1)

1. **Add `record_ty` to schema::mapping::Model**
   ```rust
   // In crates/toasty-core/src/schema/mapping/model.rs
   pub struct Model {
       // ... existing fields ...
       pub record_ty: stmt::Type,
   }
   ```

2. **Define Core Typed Wrapper**
   ```rust
   // In crates/toasty/src/engine/typed.rs
   pub struct Typed<T> {
       value: T,
       ty: stmt::Type,
   }
   ```

3. **Add Type Construction Helpers**
   ```rust
   impl Planner<'_> {
       fn build_typed_statement(&self, stmt: stmt::Statement) -> Typed<stmt::Statement>
       fn build_include_aware_model_type(&self, model: &Model, includes: &[Path]) -> stmt::Type
   }
   ```

### Phase 2: Planner Integration (Week 2)

1. **Update Select Planner Entry Point**
   - Build `Typed<Statement>` using cached `record_ty` + includes
   - Thread through existing pipeline

2. **Update Lowering Functions**
   ```rust
   fn lower_typed_stmt_query(&self, model: &Model, typed_stmt: &mut Typed<stmt::Statement>)
   ```

3. **Update partition_returning**
   - Accept pre-computed type from `Typed<T>` instead of inferring
   - Maintain type accuracy during partitioning

### Phase 3: Variable System Integration (Week 2-3)

1. **Update VarTable**
   ```rust
   pub fn register_typed_var(&mut self, typed_val: Typed<impl Into<stmt::Type>>) -> plan::VarId
   ```

2. **Thread Typed Variables**
   - Associate actions work with typed variables
   - Type information flows through execution

### Phase 4: Action Integration & Testing (Week 3-4)

1. **Update Actions to Preserve Types**
   - Associate actions update both values and types
   - Validation that types match runtime values

2. **Comprehensive Testing**
   - All include combinations
   - Cross-database compatibility
   - Performance validation

### Phase 5: Migration & Cleanup (Week 4+)

1. **Gradual Migration**
   - Convert functions one at a time to use `Typed<T>`
   - Maintain backward compatibility during transition

2. **Performance Optimization**
   - Minimize type computations
   - Cache frequently-used types

## Benefits

### Immediate
- **Fixes Include Feature**: Multiple includes work correctly
- **Better Error Messages**: Type mismatches provide clearer errors
- **Type Safety**: Catch type errors during planning instead of runtime

### Long-term
- **Feature Enablement**: Foundation for advanced features like nested includes, conditional loading
- **Performance**: Better optimization opportunities with rich type information
- **Maintainability**: Clearer separation between app and table concerns
- **Debugging**: Rich type context makes issues easier to diagnose

## Follow-ups: Type Inference Removal

The current codebase has extensive type inference code (`infer_expr_ty`, `infer_eval_expr_ty`) that is brittle and flawed. With the `Typed<T>` approach, we may be able to eliminate most or all type inference.

### Current Type Inference Usage

Survey of all type inference calls in the codebase:

#### 1. **Planning Phase**
- **`partition_returning()` (output.rs:31)**: Infers return type from expression
  - **After change**: ❌ **NOT NEEDED** - type comes from `Typed<Returning>`
- **Include fixup (select.rs:51,57)**: Gets record type for included models  
  - **After change**: ❌ **NOT NEEDED** - use cached `record_ty` instead
- **`infer_model_record_type()` (ty.rs:22)**: Builds record type from model fields
  - **After change**: ❌ **NOT NEEDED** - use cached `record_ty` instead

#### 2. **Expression Partitioning** 
During `partition_returning()`, individual expressions get partitioned:
- **Record fields (output.rs:96)**: Infers type of fields being partitioned
- **Cast expressions (output.rs:141)**: Infers type before casting  
- **Projections (output.rs:151)**: Infers base type before projection
- **Enum decode (output.rs:187)**: Infers base type before decoding

**After change**: ⚠️ **POSSIBLY STILL NEEDED** - these are for eval expressions during partitioning

#### 3. **Eval Function Creation**
- **`eval::Func::from_stmt()` (eval.rs:28,84,98)**: Creates evaluation functions
- **After change**: ⚠️ **POSSIBLY STILL NEEDED** - for pure evaluation expressions

### Analysis: Can We Eliminate All Type Inference?

**Definitely Eliminated:**
- ❌ Main `partition_returning()` type inference
- ❌ Model record type inference  
- ❌ Include-related type inference

**Still Needed?**
The remaining inference is for **eval expressions** - expressions that get evaluated in-memory rather than sent to the database.

**Potential Path to Complete Elimination:**
1. **Track types through partitioning**: When an expression gets partitioned, preserve type information
2. **Type-aware eval construction**: Pass known types to eval functions instead of inferring
3. **Eliminate remaining inference**: The inference code becomes unused

### Recommendation

After implementing `Typed<T>`, audit the remaining `infer_expr_ty()` calls. We estimate **~80% of type inference can be immediately eliminated**, with the potential to remove it entirely by threading type information through expression partitioning.

This would eliminate a major source of bugs and complexity in the query engine.

## Conclusion

The type system redesign addresses fundamental architectural limitations in the current query engine. By maintaining semantic type information throughout the entire pipeline, we can correctly handle features like includes while providing a solid foundation for future enhancements.

The `Typed<T>` wrapper approach provides a clean, efficient solution that leverages existing infrastructure while opening the door to eliminate the brittle type inference system entirely.