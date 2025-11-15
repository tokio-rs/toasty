## Investigation Results: Issue Confirmed ✓

I've investigated this issue and can confirm it's **valid**. Toasty currently allows incompatible storage types to be specified without any compile-time validation.

### Reproduction

The following code compiles successfully without any errors or warnings:

```rust
#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    // ✓ This makes sense: String → varchar
    #[column(type = varchar(50))]
    name: String,

    // ✗ This doesn't make sense: i32 → varchar
    // But it compiles without error!
    #[column(type = varchar(10))]
    age: i32,
}
```

### Root Cause

The issue is in `crates/toasty-codegen/src/schema/field.rs`:

1. **Lines 139-147**: The `#[column]` attribute is parsed and stored without validation
2. **Lines 153-157**: Only validates that relation fields don't have storage types
3. **No validation exists** for primitive type compatibility

In `crates/toasty-codegen/src/expand/schema.rs` (lines 72-83), the storage type is used directly:

```rust
let storage_ty = match &field.attrs.column {
    Some(Column { ty: Some(ty), ..}) => {
        quote!(Some(#ty))  // No validation here!
    }
    _ => quote!(None),
};
```

### Expected Behavior

The derive macro should produce a compile-time error:

```
error: incompatible storage type
  --> src/models/user.rs:12:7
   |
12 |     #[column(type = varchar(10))]
   |       ^^^^^^^^^^^^^^^^^^^^^^^^^^^ storage type `varchar` is incompatible with field type `i32`
   |
   = note: varchar storage is only valid for String fields
   = help: remove the column attribute to use default storage
```

### Valid Type Mappings

Currently, only `varchar(n)` storage is supported (defined in `column.rs:65-67`):

| Rust Type | Valid Storage Types |
|-----------|-------------------|
| `String`  | ✓ `varchar(n)` |
| `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64` | ✗ varchar not valid |
| `uuid::Uuid` | ✗ varchar not valid |
| `Id<T>` | ✗ varchar not valid |

### Implementation Approach

Add validation in `crates/toasty-codegen/src/schema/field.rs` after line 147:

```rust
// After parsing the column attribute, validate compatibility
if let (Some(column), FieldTy::Primitive(field_ty)) = (&attrs.column, &ty) {
    if let Some(storage_ty) = &column.ty {
        validate_storage_type_compatibility(storage_ty, field_ty, &field)?;
    }
}
```

The validation should check:
- `varchar(n)` → only allowed for `String` fields
- Future storage types → validate against appropriate Rust types

### Related Code Locations

- `crates/toasty-codegen/src/schema/column.rs` - Column attribute parsing
- `crates/toasty-codegen/src/schema/field.rs` - Field processing
- `crates/toasty-codegen/src/expand/schema.rs` - Schema code generation
- `crates/toasty/src/stmt/primitive.rs` - Primitive trait implementations

### Test Coverage

Existing test `tests/tests/field_column_type.rs` only tests valid usage (String with varchar). No tests currently verify that invalid combinations are rejected.
