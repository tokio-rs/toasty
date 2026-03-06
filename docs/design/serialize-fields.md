# Serialized Field Implementation Design

Builds on the `#[serialize]` bookkeeping already in place (attribute parsing,
`SerializeFormat` enum, `FieldPrimitive.serialize` field). This document covers
the runtime serialization/deserialization codegen.

## User-Facing API

```rust
#[derive(Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,

    #[serialize(json)]
    tags: Vec<String>,

    #[serialize(json)]
    metadata: Option<HashMap<String, String>>,
}
```

Fields annotated with `#[serialize(json)]` are stored as JSON text in a single
database column. The field's Rust type must implement `serde::Serialize` and
`serde::DeserializeOwned`. The database column type defaults to `String`/`TEXT`.

## Value Encoding

A serialized field stores a JSON string in the database. The value stream uses
`Value::String` for serialized fields, not the field's logical Rust type.

```
Rust value ──serde_json::to_string──► Value::String(json) ──► DB column (TEXT)
DB column (TEXT) ──► Value::String(json) ──serde_json::from_str──► Rust value
```

## Schema Changes

For serialized fields, `field_ty` bypasses `<T as Primitive>::field_ty()` and
constructs `FieldPrimitive` directly with `ty: Type::String`. The user's Rust
type `T` does not need to implement `Primitive` — it only needs `Serialize` +
`DeserializeOwned`.

```rust
// Non-serialized (existing behavior):
field_ty = <T as Primitive>::field_ty(storage_ty, serialize);
nullable = <T as Primitive>::NULLABLE;

// Serialized:
field_ty = FieldTy::Primitive(FieldPrimitive {
    ty: Type::String,
    storage_ty,
    serialize,
});
nullable = is_option_type(ty);  // detected at the syn::Type level
```

`is_option_type` checks whether the outermost type path segment is `Option`.

## Codegen Changes

### `Primitive::load` / `Model::load`

For serialized fields, the generated load code reads a `String` from the record
and deserializes it:

```rust
// Required field:
field_name: {
    let json_str = <String as Primitive>::load(record[i].take())?;
    serde_json::from_str(&json_str)
        .map_err(|e| Error::from_args(
            format_args!("failed to deserialize field '{}': {}", "field_name", e)
        ))?
},

// Option<T> field:
field_name: {
    let value = record[i].take();
    if value.is_null() {
        None
    } else {
        let json_str = <String as Primitive>::load(value)?;
        Some(serde_json::from_str(&json_str)
            .map_err(|e| Error::from_args(
                format_args!("failed to deserialize field '{}': {}", "field_name", e)
            ))?)
    }
},
```

Non-serialized fields are unchanged: `<T as Primitive>::load(record[i].take())?`.

### Reload (root model and embedded)

Reload match arms follow the same pattern: load as `String`, then deserialize.
For `Option<T>`, check null first.

### Create builder setters

Serialized field setters accept the concrete Rust type (not `impl IntoExpr<T>`,
since `T` does not implement `IntoExpr`) and serialize to a `String` expression:

```rust
// Required field:
pub fn field_name(mut self, field_name: FieldType) -> Self {
    let json = serde_json::to_string(&field_name).expect("failed to serialize");
    self.stmt.set(index, <String as IntoExpr<String>>::into_expr(json));
    self
}

// Option<T> field:
pub fn field_name(mut self, field_name: Option<InnerType>) -> Self {
    match &field_name {
        Some(v) => {
            let json = serde_json::to_string(v).expect("failed to serialize");
            self.stmt.set(index, <String as IntoExpr<String>>::into_expr(json));
        }
        None => {
            self.stmt.set(index, Expr::<String>::from_value(Value::Null));
        }
    }
    self
}
```

### Update builder setters

Same pattern as create: accept the concrete type, serialize to JSON, store as
`String` expression.

## Dependencies

`serde_json` is added as an optional dependency of the `toasty` crate, gated
behind the existing `serde` feature:

```toml
# crates/toasty/Cargo.toml
[features]
serde = ["dep:serde_core", "dep:serde_json"]

[dependencies]
serde_json = { workspace = true, optional = true }
```

Generated code references `serde_json` through the codegen support module:

```rust
// crates/toasty/src/lib.rs, in codegen_support
#[cfg(feature = "serde")]
pub use serde_json;
```

If a user writes `#[serialize(json)]` without enabling the `serde` feature, the
generated code fails to compile because `codegen_support::serde_json` does not
exist. The compiler error points at the generated `serde_json::from_str` call.

## Files Modified

| File | Change |
|------|--------|
| `crates/toasty/Cargo.toml` | Add `serde_json` optional dep, update `serde` feature |
| `crates/toasty/src/lib.rs` | Re-export `serde_json` in `codegen_support` |
| `crates/toasty-codegen/src/expand/schema.rs` | Construct `FieldPrimitive` directly for serialized fields with `ty: String` |
| `crates/toasty-codegen/src/expand/model.rs` | Deserialize in `expand_load_body()` and `expand_embedded_reload_body()` |
| `crates/toasty-codegen/src/expand/create.rs` | Serialize in create setter for serialized fields |
| `crates/toasty-codegen/src/expand/update.rs` | Serialize in update setter, deserialize in reload arms |
| `crates/toasty-driver-integration-suite/Cargo.toml` | Add `serde`, `serde_json` deps, enable `serde` feature |
| `crates/toasty-driver-integration-suite/src/tests/serialize.rs` | Integration tests |

## Integration Tests

New file `serialize.rs` in the driver integration suite. Test cases:

- Round-trip a `Vec<String>` field through create and read-back
- Round-trip an `Option<T>` serialized field with `Some` and `None` values
- Update a serialized field and verify the new value persists
- Round-trip a custom struct with `serde::Serialize + DeserializeOwned`
