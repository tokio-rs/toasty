# Enums and Embedded Structs — Remaining Work

Addresses [Issue #280](https://github.com/tokio-rs/toasty/issues/280).

Base support for embedded structs and data-carrying enums has shipped. Three
extensions remain, none started except where noted:

1. **Tuple variants** — unnamed variant fields (`Phone(String, String)`).
2. **Shared columns across variants** — one column backing a field that
   several variants carry.
3. **Within-variant partial updates** — `stmt::patch` on a variant+field path
   (the path machinery exists; variant-gating does not).

This document designs only what is left. The "Already shipped" section is
context, kept deliberately short.

## Already shipped (context, do not re-design)

CRUD on embedded structs and data-carrying enums, variant filtering, and
`stmt::patch` on embedded **structs** all work and are covered by the
`embed_struct.rs` / `embed_enum_*.rs` suites.

```rust
// Data-carrying enum, struct variants only (tuple variants rejected today).
#[derive(toasty::Embed)]
enum Contact {
    #[column(variant = 1)]
    Email { address: String },
    #[column(variant = 2)]
    Phone { country: String, number: String },
}

// Variant filtering — closure form, gates on the discriminator implicitly.
User::all().filter(
    User::fields().contact().phone().matches(|p| p.number().eq("555-1234"))
);

// stmt::patch on an embedded STRUCT field — works, including nested.
user.update()
    .address(stmt::patch(Address::fields().city(), "Seattle"))
    .exec(&mut db).await?;
```

The accessor `EnumFields::variant().field()` (e.g.
`Contact::fields().phone().number()`) already returns a **variant-rooted
`Path`** — built via `Path::into_variant` (`toasty/src/stmt/path.rs:88`). In
filter context that root injects an `is_variant` AND-gate
(`path.rs:137` `build_filter`). The update path ignores the root — see §3.

---

## 1. Tuple variants

Unnamed variant fields are rejected at
`toasty-macros/src/model/schema/model.rs:551`:

```rust
if f.unnamed.len() > 1 {
    return Err(/* "tuple structs (besides new-type) are not supported" */);
}
```

Downstream codegen is already tuple-aware: `Primitive::load` /
`IntoExpr` in `model/expand/embedded_enum.rs` emit tuple construction and
destructuring, and `schema/field.rs:203` synthesizes `_0` / `_1` names. The
gap is column naming and lifting the rejection.

### Design

A tuple variant maps each unnamed field to its own **nullable** column. The
column holds the field's value when the row's discriminator matches the
variant, NULL otherwise — identical storage to a struct variant, only the
default name differs.

Default column name: `{enum_field}_{variant}_{index}` (struct variants use
`{enum_field}_{variant}_{field_name}`; tuple variants substitute the
positional index for the missing name).

```rust
#[derive(toasty::Embed)]
enum Contact {
    #[column(variant = 1)]
    Phone(String, String),
}
// Columns: contact, contact_phone_0, contact_phone_1
```

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

- **Lift the rejection** at `model/schema/model.rs:551` for unnamed fields
  **inside enum variants only**. Standalone non-newtype tuple structs stay
  rejected (no column-naming story — see Out of scope).
- **Emit column names.** Variant-field expansion currently leaves
  `storage: None` for unnamed fields (`model/expand/embedded_enum.rs:299`,
  `expand/schema.rs:100`). Generate `{enum_field}_{variant}_{index}` and honor
  a per-field `#[column("name")]` override (attribute parsing for tuple fields
  does not exist yet).
- **Nullable columns.** Same as struct-variant fields — only the matching
  variant writes a value.
- **Zero-field tuple variants** (`Foo()`) collapse to the unit-variant case.

---

## 2. Shared columns across variants

Today every variant field gets a distinct column. `map_field_enum`
(`toasty-core/src/schema/builder/table.rs:895`) creates one `MapField` per
variant; `column_name` (`table.rs:1149`) yields
`creature_human_name` and `creature_animal_name` as separate columns.

### Design

Multiple variants share one column by giving the same `#[column("name")]` to a
field in each variant. `#[column("name")]` already parses into
`FieldName::storage` (`model/expand/schema.rs:108`) — nothing merges matching
names.

```rust
#[derive(toasty::Embed)]
enum Creature {
    #[column(variant = 1)]
    Human  { #[column("name")] name: String, profession: String },
    #[column(variant = 2)]
    Animal { #[column("name")] name: String, species: String },
}
// Columns:
//   creature                    (discriminator)
//   creature_name               (shared, nullable)
//   creature_human_profession
//   creature_animal_species
```

The shared column is **nullable**: only variants that declare it write a
value, and readers must tolerate NULL even when every defined variant happens
to write it.

A field on the enum resolves to the shared column directly, with **no
variant gate** — that is the point of sharing, cross-variant queries:

```rust
// Any creature named "Bob", regardless of variant.
Character::all().filter(Character::fields().creature().name().eq("Bob"));

// Variant-specific still uses the gated closure form.
Character::all().filter(
    Character::fields().creature().human().matches(|h| h.name().eq("Alice"))
);
```

### What to change

- **Merge by storage name.** In `map_field_enum` (`table.rs:895`), when two
  variant fields resolve to the same `storage_name()`, emit a single nullable
  column instead of one per variant. Each variant's encode/decode path targets
  that shared column.
- **Type-compatibility check at schema build.** Fields sharing a column must
  have the **identical** primitive type. **Decision (resolves prior open
  question):** v1 requires an exact type match — `i32` in one variant and
  `i64` in another is a build error, not a silent widen. Silent coercion and
  an explicit `#[column(type = ...)]` override are out of scope.
- **Cross-variant accessor.** Generate a field accessor on the enum fields
  struct (`creature().name()`) that produces an **un-gated** model-rooted
  `Path` to the shared column. This is distinct from the existing
  `creature().human().name()` accessor, which is variant-rooted and gates on
  the discriminator. Codegen lives alongside the per-variant accessors in
  `model/expand/embedded_enum.rs`.

---

## 3. Within-variant partial updates via `stmt::patch`

Partial: the path machinery and `stmt::patch` exist and work for embedded
structs; the variant gate is dropped on the update path.

`stmt::patch` (`toasty/src/stmt/assignment.rs:370`) reads only
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

So a variant-rooted path loses its discriminator context, and the assignment
would write the column unconditionally — wrong for a row whose discriminator
does not match the patched variant.

### Design

**API — reuse the existing accessor, do not invent `variants()`/`VARIANTS`.**
The accessor that already produces variant-rooted paths for filters
(`Contact::fields().phone().number()`) is exactly what `stmt::patch` needs.
One accessor, two contexts:

```rust
user.update()
    .contact(stmt::patch(
        Contact::fields().phone().number(),  // variant-rooted Path
        "555-1234",
    ))
    .exec(&mut db).await?;
```

(Earlier drafts of this doc used `Contact::variants().phone().number()` — that
API does not exist and should not be built. Filter and patch share the
`fields().variant().field()` accessor.)

**Behavior.** A variant+field patch updates one field of the named variant and
leaves the discriminator unchanged. It applies only to rows whose current
discriminator matches the variant; rows of any other variant pass through
untouched. Switching variants requires full replacement
(`.contact(Contact::Phone { .. })`). A patch never writes the discriminator
column.

### What to change

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
  only. **Decision (resolves prior open question):** add a
  `Capability::variant_conditional_update`; the DynamoDB driver leaves it
  unset, and `engine/verify.rs` rejects a variant-gated patch on DynamoDB with
  `unsupported_feature` (mirrors `native_ilike`). A native DynamoDB
  conditional `UpdateExpression` is future work.
- **Tests.** No within-variant patch tests exist (`embed_enum_*` updates are
  all full-value replacement). Add a `driver_test` covering: patch a field on
  the matching variant (changes), the same patch on a row of another variant
  (no-op), discriminator untouched, and the SQL/DynamoDB capability split.

---

## Out of scope

- **Non-newtype tuple structs outside enums.** `#[derive(Embed)]` on a tuple
  struct has no column-naming story; convert to a named struct.
- **Variant switching via patch.** `stmt::patch` mutates within the current
  variant; changing variant uses full replacement.
- **Shared-column type coercion / `#[column(type = ...)]`.** v1 requires
  identical types (§2).
- **Native DynamoDB within-variant patch.** Capability-gated off in v1 (§3).
- **DynamoDB index shapes for tuple/shared columns.** Both reuse existing
  per-column encoding, so existing GSI rules apply.

## Alternatives considered (settled, do not revisit)

- **Per-variant nested-closure update builder**
  (`.with_contact(|c| c.phone(|p| ...))`) — rejected; each level generated a
  builder type duplicating the path infrastructure. `stmt::patch` reuses the
  typed-path accessors.
- **JSON-serialized tuple variants** — rejected; blocks per-field indexes and
  filters.
