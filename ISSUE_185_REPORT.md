# Issue #185: Missing Compile-Time Validation for Storage Type Compatibility

## Summary

Toasty currently does not validate that the storage type specified in `#[column(type = ...)]` is compatible with the Rust field type. This allows developers to specify nonsensical type combinations (e.g., varchar storage for integer fields) that compile successfully but are semantically incorrect.

## Current Behavior

The `#[column(type = ...)]` attribute can be applied to any primitive field without any validation that the storage type makes sense for that field's Rust type.

### Example of the Problem

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

**This code compiles successfully without warnings or errors.**

## Expected Behavior

The derive macro should validate type compatibility and produce a compile-time error for incompatible combinations:

```
error: incompatible storage type
  --> src/models/user.rs:12:7
   |
12 |     #[column(type = varchar(10))]
   |       ^^^^^^^^^^^^^^^^^^^^^^^^^^^ storage type `varchar` is incompatible with field type `i32`
   |
   = note: varchar storage is only valid for String fields
   = help: remove the column attribute to use default storage, or specify a compatible type
```

## Root Cause Analysis

### Code Location

The issue stems from the code generation in `crates/toasty-codegen/src/schema/field.rs` and `crates/toasty-codegen/src/expand/schema.rs`:

1. **Parsing** (`field.rs:139-147`): The `#[column]` attribute is parsed and stored without validation
2. **Code Generation** (`schema.rs:72-83`): The storage type is used directly without checking compatibility

```rust
// In expand/schema.rs
let storage_ty = match &field.attrs.column {
    Some(Column { ty: Some(ty), ..}) => {
        quote!(Some(#ty))  // No validation here!
    }
    _ => quote!(None),
};
```

3. **Partial Validation** (`field.rs:153-157`): Only checks that relation fields don't have storage types:

```rust
if ty.is_some() && attrs.column.is_some() {
    errs.push(syn::Error::new_spanned(
        field,
        "relation fields cannot have a database type",
    ));
}
```

### Type System Architecture

Toasty has two separate type representations:

- **`stmt::Type`**: Logical type inferred from Rust field type (e.g., `i32` → `stmt::Type::I32`)
- **`db::Type`**: Physical storage type specified by user (e.g., `varchar(10)` → `db::Type::VarChar(10)`)

These are stored separately in the schema:

```rust
FieldPrimitive {
    ty: <#ty as toasty::stmt::Primitive>::ty(),  // From Rust type
    storage_ty: #storage_ty,                      // From #[column(type = ...)]
}
```

**The problem**: No validation ensures these are compatible.

## Valid Type Mappings

Currently, only `varchar` storage type is supported in the parser (`column.rs:65-67`). This should only be valid for `String` fields:

| Rust Type | Valid Storage Types |
|-----------|-------------------|
| `String`  | `varchar(n)`, custom string types |
| `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64` | (none currently - should maybe support custom integer types?) |
| `uuid::Uuid` | (none currently) |
| `Id<T>` | (none currently) |

## Reproduction

A minimal reproduction case is included in `tests/tests/issue_185_repro.rs`:

```bash
cargo test --test issue_185_repro --no-run
```

This compiles successfully, demonstrating the issue.

## Implementation Approach

The validation should be added in `crates/toasty-codegen/src/schema/field.rs` after parsing the column attribute:

```rust
// After line 147, add validation
if let (Some(column), FieldTy::Primitive(field_ty)) = (&attrs.column, &ty) {
    if let Some(storage_ty) = &column.ty {
        validate_storage_type_compatibility(storage_ty, field_ty, &field)?;
    }
}
```

The validation function should check:

1. **varchar** storage → only allowed for `String` fields
2. Future storage types (if added) → validate against appropriate Rust types

## Potential Edge Cases

1. **Generic types**: How should `Option<String>` be handled? (Currently the outer Option is transparent)
2. **Custom types**: Should custom storage types skip validation?
3. **Future storage types**: If integer/boolean storage types are added, they need validation rules

## Related Code

- `crates/toasty-codegen/src/schema/column.rs` - Column attribute parsing
- `crates/toasty-codegen/src/schema/field.rs` - Field processing
- `crates/toasty-codegen/src/expand/schema.rs` - Schema code generation
- `crates/toasty/src/stmt/primitive.rs` - Primitive trait implementations
- `crates/toasty-core/src/schema/db/ty.rs` - Database storage types
- `crates/toasty-core/src/stmt/ty.rs` - Statement types

## Additional Notes

The existing test `tests/tests/field_column_type.rs` only tests valid usage (String with varchar). No tests currently verify that invalid combinations are rejected.
