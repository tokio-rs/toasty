# Path field metadata

## Summary

Typed paths gain three read-only metadata accessors: `field_name()`,
`is_nullable()`, and `is_unique()` on `Path<M, T>`. They answer "what field
does this path point at?" without a `Db`. Two re-exports:
`toasty::stmt::CorePath` and `toasty::stmt::ValueRecord`, let callers name
the untyped types behind them without depending on `toasty-core`.

## Motivation

A typed path identifies a field but erases its metadata: the app name,
whether it is nullable, whether it is unique. That data lives in the app
schema, reachable today only through a built `Db` or a direct `toasty-core`
dependency. Generic code over models needs the answers from the path
itself: a table renderer labeling columns and flagging optional ones, a
keyset-pagination helper refusing a non-unique cursor column.

## User-facing API

Three methods on `Path<M, T>` where `M: Model`:

- `field_name() -> String` the app-level (Rust) name of the field.
- `is_nullable() -> bool` whether the field accepts `NULL`.
- `is_unique() -> bool` whether the field is the target of a single-field
  unique index: `#[unique]` fields, enum-level `#[unique(variant::field)]`
  references, and primary-key fields of single-field primary keys.
  Components of composite unique indices or composite primary keys are not
  unique on their own.

```rust
// #[unique] on User.email; enum-level #[unique(email::address)] on Contact
assert_eq!(User::fields().email().field_name(), "email");
assert!(User::fields().email().is_unique());
assert!(User::fields().bio().is_nullable());
assert!(User::fields().contact().email().address().is_unique());
```

All three resolve through embedded structs and embedded-enum variants to
any depth — a struct inside a variant, an enum inside a variant.

The re-exports:

- `toasty::stmt::CorePath` — the untyped `toasty_core::stmt::Path` that a
  typed path converts into.
- `toasty::stmt::ValueRecord` — the record value type.

Supporting addition in `toasty-core`: `app::ModelSet::get(id)` returns the
model with the given `ModelId`, if present.

## Behavior

- No `Db` required. Each call builds the app schema for `M`'s reachable
  models and resolves the path against it. These are one-off probes, not
  per-row helpers.
- `is_unique()` scans the owning model's `app::Index` entries (there is no
  per-field unique flag) and matches only single-field unique indices.
- Panics, matching the crate's `_unwrap`-on-misuse style, when the path
  does not end at a field, or when the projection crosses a relation. A
  path may end at a relation field; projecting through one panics.

## Edge cases

- Variant-rooted paths resolve variant-local indices. The discriminant
  offset the engine applies in `Path::into_stmt` does not apply here.
- `field_name()` is the Rust field name, not the database column.
  Flattened embed columns and storage overrides live in the mapping layer.

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
