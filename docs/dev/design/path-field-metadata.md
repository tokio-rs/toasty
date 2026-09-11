# Path field metadata

## Summary

Typed paths gain three read-only metadata accessors: `field_name()`,
`is_nullable()`, and `is_unique()` on `Path<M, T>`. They answer "what field
does this path point at?" without a `Db`. Two re-exports —
`toasty::stmt::CorePath` and `toasty::stmt::ValueRecord` — let callers name
the untyped types behind them without depending on `toasty-core`.

## Motivation

A typed path identifies a field but erases its metadata: the app name,
whether it is nullable, whether it is unique. That data lives in the app
schema, reachable today only through a built `Db` or a direct `toasty-core`
dependency. Generic code over models needs the answers from the path
itself: a table renderer labeling columns and flagging optional ones, a
keyset-pagination helper flagging a unique-index candidate (cursor safety
needs additional storage-nullability checks; see Behavior). A first
consumer is an admin-panel form builder deriving `required` and `unique`
defaults from the bound field.

## User-facing API

Three methods on `Path<M, T>` where `M: Model`:

- `field_name() -> String` the app-level (Rust) name of the field.
  Panics on the unnamed `inner` field of a tuple-newtype embed, which has
  no app-level name.
- `is_nullable() -> bool` whether the leaf field is `Option`-marked (not
  whether storage accepts `NULL`; see Behavior). Available when the leaf
  target is a storable field type (`T: Field`); relation terminals and model
  roots have no method, and an embed root reports `false` without a leaf
  field. List targets are never `Option`-wrapped and report `false`. Read
  from the leaf type's `Field::NULLABLE`, so it needs no schema walk and is
  only sound for generated accessors (hand-built `path_field::<U>` / `chain`
  paths must supply the field's `ExprTarget` as `U`).
- `is_unique() -> bool` whether the field is the target of a single-field
  unique index (index membership only, not a global-uniqueness guarantee):
  `#[unique]` fields, enum-level `#[unique(variant::field)]`
  references, enum-level `#[unique(shared)]` references (true for every
  `#[shared(shared)]` member, which share one column), and primary-key
  fields of single-field primary keys.
  Components of composite unique indices or composite primary keys are not
  unique on their own.

```rust
// #[unique] on User.email; enum-level #[unique(email::address)] on Contact
assert_eq!(User::fields().email().field_name(), "email");
assert!(User::fields().email().is_unique());
assert!(User::fields().bio().is_nullable());
assert!(User::fields().contact().email().address().is_unique());
```

`field_name()` and `is_unique()` resolve through embedded structs,
embedded-enum variants, and `#[document]` embeds to any depth — a struct
inside a variant, an enum inside a variant, a document inside a document.
`is_nullable()` needs no resolution: the app schema generates its `nullable`
flag from the field type's `Field::NULLABLE`, which the typed path's target
type already carries.

The re-exports:

- `toasty::stmt::CorePath` — the untyped `toasty_core::stmt::Path` that a
  typed path converts into.
- `toasty::stmt::ValueRecord` — the record value type.

Supporting addition in `toasty-core`: `app::ModelSet::get(id)` returns the
model with the given `ModelId`, if present.

## Behavior

- No `Db` required. `field_name()` and `is_unique()` each build the app
  schema for `M`'s reachable models and resolve the path against it; these
  are one-off probes, not per-row helpers. `is_nullable()` reads the leaf
  type and never builds a schema.
- `is_nullable()` reports the leaf field's `Option` marker only, not storage
  `NULL`s from a nullable parent embed or an inactive enum variant. For a
  list-targeted path (`Vec<T>` fields) it is always `false`; an
  `Option<Vec<T>>` field keeps `Option<Vec<T>>` as its path target and is
  covered by the `T: Field` impl.
- `is_unique()` scans the owning model's `app::Index` entries (there is no
  per-field unique flag) and matches only single-field unique indices.
  Enum-level `#[unique(shared)]` stores the first `#[shared(shared)]` member
  only, so members compare by shared identifier, not `FieldId`. Reports index
  membership only: `NULL`s do not conflict in unique indices (SQL treats
  them as distinct; DynamoDB skips the index entry). `true` implies globally
  unique values only when the column cannot be `NULL` (non-optional leaf, no
  nullable parent embed or enum variant crossed).
  Variant columns are storage-nullable by construction, including `#[shared]`
  columns, so a variant path can permit duplicate `NULL`s even when
  `is_nullable()` is `false`.
- `is_unique()` reports `false` inside a `#[document]` embed: the app-level
  index has no database backing.
- Panics, matching the crate's `_unwrap`-on-misuse style, when the path
  does not end at a field, when the projection crosses a relation, or when
  `field_name()` targets the unnamed `inner` field of a tuple-newtype
  embed. A
  path may end at a relation field; projecting through one panics.
  Projecting through embedded (including `#[document]`) and enum steps is
  supported.

## Edge cases

- Variant-rooted paths resolve variant-local indices. The discriminant
  offset the engine applies in `Path::into_stmt` does not apply here.
- `field_name()` is the Rust field name, not the database column.
  Flattened embed columns and storage overrides live in the mapping layer.
  The `inner` field of a tuple-newtype embed is transparent (it takes the
  parent field's column) and has no app-level name, so `field_name()`
  panics there; `is_nullable()` and `is_unique()` still work.

## Driver integration

None. Read-only views over the app schema; drivers see no changes.

## Alternatives considered

- **Reuse `app::Schema::resolve_field_path`.** It needs a fully linked
  `Schema` (relation linking on every call) and resolves the engine's path
  dialect (discriminant steps), not the typed dialect (variant-local
  indices).
- **Macro-emitted const tables.** `Path<M, T>` erases field identity at the
  type level, so per-field consts cannot attach to paths. Compile-time
  tables remain a possible follow-up to remove the per-call schema build.

## Out of scope

- Database column names — the mapping layer needs a compiled schema.
- Caching the per-call schema build — deferred until a hot path needs it.
