# toasty-codegen Component Context

## Purpose
Generates Rust code from the `#[derive(Model)]` procedural macro. This crate transforms user-defined structs into full-featured ORM models with query builders, CRUD operations, and relationship handling.

## Key Responsibilities

### Schema Parsing (`src/schema/`)
Parses attributes and builds internal schema representation:
- **Model attributes**: `#[model(table = ...)]`, key definitions
- **Field attributes**: `#[key]`, `#[auto]`, `#[unique]`, `#[index]`  
- **Relation attributes**: `#[has_many]`, `#[belongs_to]`, `#[has_one]`
- **Type mapping**: Rust types to schema primitives

### Code Expansion (`src/expand/`)
Generates implementations for parsed models:
- **model.rs**: Core `Model` trait, load(), id()
- **query.rs**: Query builder structs and methods
- **create.rs**: Insert builders with field setters
- **update.rs**: Update builders with field mutations
- **relation.rs**: Relationship accessor methods
- **fields.rs**: Field path expressions
- **filters.rs**: Type-safe filter methods

## Common Change Patterns

### Adding a New Model Attribute
1. Define attribute structure in `schema/model_attr.rs`
2. Parse in `schema/model.rs::from_derive_input()`
3. Use in `expand/model.rs` during code generation
4. Update documentation

### Adding Query Methods
1. Define method signature in `expand/query.rs`
2. Generate implementation using quote! macro
3. Ensure proper type qualification (`#toasty::...`)
4. Add to query struct methods

### Supporting New Field Types
1. Map Rust type in `schema/ty.rs`
2. Handle in `expand/fields.rs` for getters/setters
3. Update `expand/filters.rs` for query filters
4. Generate proper conversions

## Generated Code Structure

For a model `User`, codegen creates:
```rust
// Core implementations
impl User { /* CRUD methods */ }
impl Model for User { /* trait implementation */ }
impl Relation for User { /* relationship support */ }
impl IntoExpr<User> for User { /* expression conversion */ }
impl IntoSelect for User { /* query conversion */ }

// Builder structs
struct UserQuery { /* query builder */ }
struct UserCreate { /* insert builder */ }
struct UserUpdate<'a> { /* update builder */ }
struct UserUpdateQuery { /* update query builder */ }

// Field accessors
struct UserFields;
impl UserFields { /* field path methods */ }

// Relationship types
struct Many, One, OptionOne, ManyField, OneField;
```

## Complete Attribute Support

### Model-Level Attributes
- `#[key(partition = field1, local = field2)]` - Composite keys with distributed DB support
- `#[table = "table_name"]` - Custom table name mapping

### Field-Level Attributes  
- `#[key]` - Primary key field
- `#[auto]` - Auto-generated values
- `#[unique]` - Unique constraint with index
- `#[index]` - Non-unique index
- `#[db(varchar(size))]` - Database column type override

### Relationship Attributes
- `#[belongs_to(key = source, references = target)]` - Foreign key (supports composite)
- `#[has_many(pair = back_ref)]` - One-to-many with optional back-reference
- `#[has_one]` - One-to-one with auto back-reference inference

## Important Files

- `lib.rs`: Macro entry point
- `schema/model.rs`: Model parsing logic
- `expand/model.rs`: Main expansion logic
- `expand/util.rs`: Helper functions for code generation
- `schema/ty.rs`: Type system mapping

## Code Generation Principles

1. **Fully Qualified Paths**: Always use `#toasty::Type` to avoid conflicts
2. **Type Safety**: Generate strongly-typed builders and queries
3. **Zero-Cost**: Generated code should have no runtime overhead
4. **Discoverable API**: Methods should be intuitive and IDE-friendly

## Key Patterns

### Dynamic Model IDs
Models now use runtime ID generation via `OnceLock`:
```rust
fn id() -> ModelId {
    static ID: std::sync::OnceLock<ModelId> = std::sync::OnceLock::new();
    *ID.get_or_init(|| #toasty::generate_unique_id())
}
```

### Query Builder Pattern
All query methods return builders that chain:
```rust
User::all()
    .filter_by_email("...")
    .order_by_name()
    .limit(10)
```

### Relationship Expansion
Relations generate both accessor and mutation methods:
- Accessor: `user.todos()` returns query builder
- Mutation: Create builders accept related entities

## Recent Changes Analysis

From recent commits:
- Switched from const Model::ID to method-based IDs
- Reduced glob imports for clarity
- Removed schema query generation (moved to runtime)
- Simplified field accessor generation
- Added support for more primitive types