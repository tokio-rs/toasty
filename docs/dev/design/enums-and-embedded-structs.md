# Enums and Embedded Structs — Remaining Work

Addresses [Issue #280](https://github.com/tokio-rs/toasty/issues/280).

Three extensions to embedded structs and data-carrying enums remain:

1. **Tuple variants** — unnamed variant fields (`Phone(String, String)`).
2. **Cross-variant accessor for shared columns** — an un-gated accessor for a
   field that several variants store in one column.
3. **Within-variant partial updates** — `stmt::patch` on a variant+field path.

---

## 1. Tuple variants

Unnamed variant fields are rejected in `collect_ast_fields`
(`toasty-macros/src/model/schema/model.rs:676`):

```rust
if f.unnamed.len() > 1 {
    return Err(/* "tuple structs (besides new-type) are not supported" */);
}
```

Downstream codegen is already tuple-aware: `Primitive::load` / `IntoExpr` in
`model/expand/embedded_enum.rs` emit tuple construction and destructuring, and
`schema/field.rs` synthesizes `_0` / `_1` names. The gap is column naming and
lifting the rejection.

### Design

A tuple variant maps each unnamed field to its own **nullable** column. The
column holds the field's value when the row's discriminator matches the
variant, NULL otherwise — identical storage to a struct variant, only the
default name differs. The default name is unresolved; see Open questions.

Per-field `#[column("name")]` override:

```rust
#[derive(toasty::Embed)]
enum Contact {
    #[column(variant = 1)]
    Phone(
        #[column("phone_country")] String,
        #[column("phone_number")] String,
    ),
}
// Columns: contact, contact_phone_country, contact_phone_number
```

### What to change

- **Lift the rejection** for unnamed fields **inside enum variants only**.
  Standalone non-newtype tuple structs stay rejected (no column-naming story —
  see Out of scope).
- **Emit column names.** Variant-field expansion leaves `storage: None` for
  unnamed fields (`model/expand/embedded_enum.rs`, `expand/schema.rs`).
  Generate the default name and honor a per-field `#[column("name")]` override
  (attribute parsing for tuple fields does not exist yet).
- **Nullable columns.** Same as struct-variant fields — only the matching
  variant writes a value.
- **Zero-field tuple variants** (`Foo()`) collapse to the unit-variant case.

---

## 2. Cross-variant accessor for shared columns

`#[shared(<ident>)]` merges a field declared by several variants into one
nullable column, named after the shared identifier:

```rust
#[derive(toasty::Embed)]
enum Creature {
    #[column(variant = 1)]
    Human  { #[shared(name)] full_name: String, profession: String },
    #[column(variant = 2)]
    Animal { #[shared(name)] nickname: String, species: String },
}
// Columns: creature, creature_name, creature_profession, creature_species
```

Both variants write `creature_name`, but there is no way to query it as one
field. Every accessor is variant-rooted and gates on the discriminator, so a
cross-variant query has to OR one predicate per variant, naming each variant's
Rust field:

```rust
Character::all().filter(
    Character::fields().creature().human().full_name().eq("Bob")
        .or(Character::fields().creature().animal().nickname().eq("Bob")),
);
```

That does not scale with variant count, and it forces callers to know which
Rust field name each variant used for the same logical field.

### Design

The shared identifier names an accessor on the enum's fields struct that
resolves to the shared column directly, with **no variant gate**:

```rust
// Any creature named "Bob", regardless of variant.
Character::all().filter(Character::fields().creature().name().eq("Bob"));
```

The accessor is named after the shared identifier, never after a variant's Rust
field name. It is distinct from `creature().human().full_name()`, which stays
variant-rooted and gated. Rows whose variant does not declare the field hold
NULL and match no equality predicate.

### What to change

- **Generate the accessor** on the enum fields struct, alongside the
  per-variant accessors in `model/expand/embedded_enum.rs`. It produces an
  un-gated, model-rooted `Path` to the shared column.
- **Name collision check.** The shared identifier must not collide with another
  shared identifier or with anything else that names an accessor on the enum's
  fields struct (variant accessors like `human()`). Nothing checks this today;
  the accessor is what makes a collision observable.

---

## 3. Within-variant partial updates via `stmt::patch`

Two pieces are missing. `#[derive(Embed)]` enums get no `fields()` associated
function, so a variant-rooted path cannot be spelled relative to the enum. And
`stmt::patch` (`toasty/src/stmt/assignment.rs:369`) reads only
`path.untyped.projection` and **discards `path.untyped.root`**:

```rust
pub fn patch<T, U>(path: Path<T, U>, value: impl Assign<U>) -> Assignment<T> {
    let inner = value.into_assignment();
    Assignment {
        kind: AssignmentKind::Patch {
            path_projection: path.untyped.projection, // root ignored
            inner: Box::new(inner.kind),
        },
        _p: PhantomData,
    }
}
```

A variant-rooted path therefore loses its discriminator context, and the
assignment would write the column unconditionally — wrong for a row whose
discriminator does not match the patched variant.

### Design

**API — reuse the existing accessor, do not invent `variants()`/`VARIANTS`.**
The accessor that already produces variant-rooted paths for filters is exactly
what `stmt::patch` needs; embedded enums expose it from their own root the way
embedded structs do (`Address::fields().city()`). One accessor, two contexts:

```rust
user.update()
    .contact(stmt::patch(
        Contact::fields().phone().number(),  // variant-rooted Path
        "555-1234",
    ))
    .exec(&mut db).await?;
```

**Behavior.** A variant+field patch updates one field of the named variant and
leaves the discriminator unchanged. It applies only to rows whose current
discriminator matches the variant; rows of any other variant pass through
untouched. Switching variants requires full replacement
(`.contact(Contact::Phone { .. })`). A patch never writes the discriminator
column.

### What to change

- **Emit `fields()` for embedded enums.** `expand_embedded_model_impls`
  (`model/expand/model.rs:256`) emits the `fields()` associated function and is
  called only from the embedded-struct path (`model/expand.rs:103`); the
  embedded-enum path (`expand.rs:229`) never calls it, so `Contact::fields()`
  does not compile. Without it, variant-rooted paths are reachable only through
  the owning model (`Character::fields().creature().human().full_name()`),
  which is rooted at the model rather than the enum and so cannot be passed to
  the enum field's setter. The `ignore` rustdoc example on `stmt::patch`
  (`assignment.rs:354`) is written against the missing API and additionally uses
  `Kind::variants()`, which is not being built; fix it when `fields()` lands.
- **Carry the variant root.** In `stmt::patch`, inspect `path.untyped.root`;
  when it is `PathRoot::Variant { variant_id, .. }`, record `variant_id` on
  the assignment (add a field to `AssignmentKind::Patch`, or a sibling
  `PatchVariant` kind). A non-variant root behaves exactly as today.
- **Lower to a guarded assignment (SQL).** For a variant-gated patch on
  column `C` with new value `E`, lowering (`engine/lower.rs`) emits
  ```sql
  C = CASE WHEN <disc_col> = <variant_discriminant> THEN E ELSE C END
  ```
  and emits **no** assignment for the discriminator column. This reuses the
  existing assignment-lowering path; drivers receive no new `Operation`.
- **DynamoDB: gate behind a capability.** Per project philosophy (don't
  emulate cross-backend differences), v1 supports within-variant patch on SQL
  only. Add a `Capability::variant_conditional_update`; the DynamoDB driver
  leaves it unset, and `engine/verify.rs` rejects a variant-gated patch on
  DynamoDB with `unsupported_feature` (mirrors `native_ilike`). A native
  DynamoDB conditional `UpdateExpression` is future work.
- **Tests.** Add a `driver_test` covering: patch a field on the matching
  variant (changes), the same patch on a row of another variant (no-op),
  discriminator untouched, and the SQL/DynamoDB capability split.

---

## Open questions

- **Default column name for tuple-variant fields** (§1, blocks
  implementation). Struct-variant fields derive `{enum_field}_{field_name}`
  with no variant segment — `Human { profession }` yields
  `creature_profession`. Applied positionally, that rule gives `contact_0` /
  `contact_1`, which collides the moment a second tuple variant appears, and
  colliding column names across variants are a build error rather than an
  implicit merge. Either tuple fields take a variant segment
  (`contact_phone_0`), breaking symmetry with struct variants, or an explicit
  `#[column("name")]` is required on every tuple field.

## Out of scope

- **Non-newtype tuple structs outside enums.** `#[derive(Embed)]` on a tuple
  struct has no column-naming story; convert to a named struct.
- **Variant switching via patch.** `stmt::patch` mutates within the current
  variant; changing variant uses full replacement.
- **Native DynamoDB within-variant patch.** Capability-gated off in v1 (§3).
- **DynamoDB index shapes for tuple columns.** Tuple fields reuse the existing
  per-column encoding, so existing GSI rules apply.

## Alternatives considered

- **Per-variant nested-closure update builder**
  (`.with_contact(|c| c.phone(|p| ...))`) — rejected; each level generated a
  builder type duplicating the path infrastructure. `stmt::patch` reuses the
  typed-path accessors.
- **JSON-serialized tuple variants** — rejected; blocks per-field indexes and
  filters.
