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

    // nullable: the column may be NULL. The Rust type must be Option<T>.
    // None maps to NULL; Some(v) is serialized as JSON.
    #[serialize(json, nullable)]
    metadata: Option<HashMap<String, String>>,

    // Non-nullable Option: the entire Option value is serialized as JSON.
    // Some(v) → `v` as JSON, None → `null` as JSON text (column is NOT NULL).
    #[serialize(json)]
    extra: Option<String>,
}
```

Fields annotated with `#[serialize(json)]` are stored as JSON text in a single
database column. The field's Rust type must implement `serde::Serialize` and
`serde::DeserializeOwned`. The database column type defaults to `String`/`TEXT`.

### Nullability

By default, serialized fields are **not nullable**. The entire Rust value —
including `Option<T>` — is serialized as-is into JSON text stored in a NOT NULL
column. This means `None` becomes the JSON text `null`, and `Some(v)` becomes
the JSON serialization of `v`.

To make the database column nullable, add `nullable` to the attribute:
`#[serialize(json, nullable)]`. When `nullable` is set:

- The Rust type **must** be `Option<T>`.
- `None` maps to a SQL `NULL` (no value stored).
- `Some(v)` serializes `v` as JSON text.

This is an explicit opt-in because the two behaviors are meaningfully different:
a user may legitimately want to serialize `None` as JSON `null` text in a NOT
NULL column (e.g., for a JSON API field where `null` is a valid value distinct
from "no row").

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

Nullability is determined by the `nullable` flag in the attribute, not by
inspecting the Rust type.

### Remove `serialize` from `Primitive::field_ty`

Today `Primitive::field_ty` accepts a `serialize` argument so it can thread
`SerializeFormat` into the `FieldPrimitive` it builds. With this design,
serialized fields never go through `Primitive::field_ty` — codegen constructs
the `FieldPrimitive` directly. That means the `serialize` parameter is dead
for all callers and should be removed.

```rust
// Primitive trait (before):
fn field_ty(
    storage_ty: Option<db::Type>,
    serialize: Option<SerializeFormat>,
) -> FieldTy;

// Primitive trait (after):
fn field_ty(storage_ty: Option<db::Type>) -> FieldTy;
```

The default implementation drops the `serialize` field from the constructed
`FieldPrimitive` (it is always `None` when going through the trait). Embedded
type overrides (`Embed`, enum) already ignore both parameters.

Codegen changes:

```rust
// Non-serialized field (calls through the trait):
field_ty = quote!(<#ty as Primitive>::field_ty(#storage_ty));
nullable = quote!(<#ty as Primitive>::NULLABLE);

// Serialized field (constructed directly):
field_ty = quote!(FieldTy::Primitive(FieldPrimitive {
    ty: Type::String,
    storage_ty: #storage_ty,
    serialize: Some(SerializeFormat::Json),
}));
nullable = #serialize_nullable; // literal bool from attribute
```

No type-level hack is needed — the `nullable` flag is parsed from the attribute
at macro expansion time and threaded through to schema registration as a
literal `bool`.

## Codegen Changes

### `Primitive::load` / `Model::load`

For serialized fields, the generated load code reads a `String` from the record
and deserializes it. The behavior depends on whether `nullable` is set:

```rust
// Non-nullable (default) — works for any T including Option<T>:
field_name: {
    let json_str = <String as Primitive>::load(record[i].take())?;
    serde_json::from_str(&json_str)
        .map_err(|e| Error::from_args(
            format_args!("failed to deserialize field '{}': {}", "field_name", e)
        ))?
},

// Nullable (#[serialize(json, nullable)]) — T must be Option<U>:
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
For nullable fields, check null first.

### Create builder setters

Serialized field setters accept the concrete Rust type (not `impl IntoExpr<T>`,
since `T` does not implement `IntoExpr`) and serialize to a `String` expression:

```rust
// Non-nullable (default) — accepts T directly (including Option<T>):
pub fn field_name(mut self, field_name: FieldType) -> Self {
    let json = serde_json::to_string(&field_name).expect("failed to serialize");
    self.stmt.set(index, <String as IntoExpr<String>>::into_expr(json));
    self
}

// Nullable (#[serialize(json, nullable)]) — accepts Option<InnerType>:
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
| `crates/toasty/src/stmt/primitive.rs` | Remove `serialize` param from `Primitive::field_ty` |
| `crates/toasty-macros/src/schema/field.rs` | Parse `nullable` flag from `#[serialize(...)]` attribute |
| `crates/toasty-macros/src/expand.rs` | Update `Embed`/enum `field_ty` overrides to drop `serialize` param |
| `crates/toasty-macros/src/expand/schema.rs` | Construct `FieldPrimitive` directly for serialized fields; remove `serialize` arg from non-serialized `field_ty` call |
| `crates/toasty-macros/src/expand/embedded_enum.rs` | Drop `serialize` arg from `field_ty` call |
| `crates/toasty-macros/src/expand/model.rs` | Deserialize in `expand_load_body()` and `expand_embedded_reload_body()` |
| `crates/toasty-macros/src/expand/create.rs` | Serialize in create setter for serialized fields |
| `crates/toasty-macros/src/expand/update.rs` | Serialize in update setter, deserialize in reload arms |
| `crates/toasty-driver-integration-suite/Cargo.toml` | Add `serde`, `serde_json` deps, enable `serde` feature |
| `crates/toasty-driver-integration-suite/src/tests/serialize.rs` | Integration tests |

## Integration Tests

New file `serialize.rs` in the driver integration suite. Test cases:

- Round-trip a `Vec<String>` field through create and read-back
- Round-trip a nullable `Option<T>` field with `Some` and `None` (SQL NULL) values
- Non-nullable `Option<T>` field: `None` round-trips as JSON `null` text (not SQL NULL)
- Update a serialized field and verify the new value persists
- Round-trip a custom struct with `serde::Serialize + DeserializeOwned`
