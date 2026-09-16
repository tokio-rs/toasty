# Path field metadata

## Summary

Typed paths gain three read-only metadata accessors: `field_name()`,
`is_nullable()`, and `is_unique()` on `Path<M, T>`. They answer "what field
does this path point at?" without a `Db`.

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

- `field_name() -> Option<String>` the app-level (Rust) name of the field,
  or `None` for the unnamed `inner` field of a tuple-newtype embed.
- `is_nullable() -> bool` whether the leaf field is `Option`-marked (not
  whether storage accepts `NULL`; see Behavior). Available whenever the path
  target implements `Field` (`Path<M, T> where T: Field`); generated
  accessors produce such targets for storable leaves, and hand-built
  `path_field::<U>` / `chain` paths pair it with whatever `U` they choose.
  List targets are never `Option`-wrapped and report `false`. Read from the
  target type's `Field::NULLABLE`, so it needs no schema walk; it is only
  sound for generated accessors, which supply the field's own target type.
- `is_unique() -> bool` whether the field is backed by a single-field unique
  index, its own or an enclosing transparent newtype's (index membership
  only, not a global-uniqueness guarantee):
  `#[unique]` fields, enum-level `#[unique(variant::field)]`
  references, enum-level `#[unique(shared)]` references (true for every
  `#[shared(shared)]` member, which share one column), and primary-key
  fields of single-field primary keys.
  Components of composite unique indices or composite primary keys are not
  unique on their own. A transparent newtype's unnamed `inner` field reports
  a unique index on any enclosing field: every layer maps to one column.

```rust
// #[unique] on User.email; enum-level #[unique(email::address)] on Contact
assert_eq!(User::fields().email().field_name().as_deref(), Some("email"));
assert!(User::fields().email().is_unique());
assert!(User::fields().bio().is_nullable());
assert!(User::fields().contact().email().address().is_unique());
```

`field_name()` and `is_unique()` resolve through embedded structs,
embedded-enum variants, `#[document]` embeds, and relations to any depth — a
struct inside a variant, an enum inside a variant, a document inside a
document, a field on a related model.
`is_nullable()` needs no resolution: the app schema generates its `nullable`
flag from the field type's `Field::NULLABLE`, which the typed path's target
type already carries.

Supporting additions in `toasty-core`: `app::ModelSet::get(id)` returns the
model with the given `ModelId`, if present; `app::ModelSet::resolve_path(path)`
resolves an untyped path against the set's models, returning the leaf field
and the first `#[document]` field crossed, or an `app::ResolveError`.

## Behavior

- No `Db` required. `field_name()` and `is_unique()` each build the app
  schema for `M`'s reachable models and resolve the path against it; these
  are one-off probes, not per-row helpers. `is_nullable()` reads the leaf
  type and never builds a schema.
- `is_nullable()` reports the leaf field's `Option` marker only, not storage
  `NULL`s from a nullable parent embed, a nullable relation crossed on the
  way to the leaf, or an inactive enum variant. For a
  list-targeted path (`Vec<T>` fields, including a field reached through a
  to-many relation) it is always `false`; an
  `Option<Vec<T>>` field keeps `Option<Vec<T>>` as its path target and is
  covered by the `T: Field` impl.
- `is_unique()` scans the `app::Index` entries of the leaf's model and of
  every enclosing transparent-newtype field (they share one column) and
  matches only single-field unique indices.
  Enum-level `#[unique(shared)]` stores the first `#[shared(shared)]` member
  only, so members compare by shared identifier, not `FieldId`. Reports index
  membership only: `NULL`s do not conflict in unique indices (SQL treats
  them as distinct; DynamoDB skips the index entry). `true` implies globally
  unique values only when the column cannot be `NULL` (non-optional leaf, no
  nullable parent embed, nullable relation, or enum variant crossed).
  Variant columns are storage-nullable by construction, including `#[shared]`
  columns, so a variant path can permit duplicate `NULL`s even when
  `is_nullable()` is `false`. A relation step reports the leaf field's index
  in its own model; values may repeat across root rows, so `true` does not
  make the path a cursor key.
- `is_unique()` reports `false` inside a `#[document]` embed: the app-level
  index has no database backing.
- Panics, matching the crate's `_unwrap`-on-misuse style, when the path
  does not end at a field or projects through something that is not a model
  (a primitive, a non-model embed, or a scalar-terminal `via`); hand-built
  paths can also panic on a variant root whose parent does not name the
  embedded enum, or on a step outside `T`'s reachable schema. A path may end
  at a relation field and may project through it to the target model.
  Projecting through embedded (including `#[document]`) and enum steps is
  supported.

## Edge cases

- Variant-rooted paths resolve variant-local indices. The discriminant
  offset the engine applies in `Path::into_stmt` does not apply here.
- `field_name()` is the Rust field name, not the database column.
  Flattened embed columns and storage overrides live in the mapping layer.
  The `inner` field of a tuple-newtype embed is transparent (it takes the
  parent field's column) and has no app-level name, so `field_name()`
  returns `None` there; `is_unique()` reports a unique index on an enclosing
  field, which constrains the same column, and `is_nullable()` reads the
  inner type.

## Driver integration

None. Read-only views over the app schema; drivers see no changes.

## Alternatives considered

- **Reuse `app::Schema::resolve_field_path`.** It resolves variant-rooted
  paths in the typed dialect, but needs a fully linked `Schema` (relation
  linking on every call), while the metadata accessors only need the models
  reachable from `M`.
- **Macro-emitted const tables.** `Path<M, T>` erases field identity at the
  type level, so per-field consts cannot attach to paths. Compile-time
  tables remain a possible follow-up to remove the per-call schema build.

## Out of scope

- Database column names — the mapping layer needs a compiled schema.
- Caching the per-call schema build — deferred until a hot path needs it.
