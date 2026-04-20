# Enums and Embedded Structs

Addresses [Issue #280](https://github.com/tokio-rs/toasty/issues/280).

## Summary

Toasty embeds enums and structs in models via `#[derive(Embed)]`. Unit and
struct-variant enums, newtypes, nested types, column renaming, discriminator
customization, variant-only filters, variant+field filters via `.matches()`,
embedded struct partial updates, and automatic transitive registration all
work today. Three extensions remain: tuple variants, columns shared across
variants, and enum within-variant partial updates via `stmt::patch`. A
related cleanup removes the `.with_<field>` closure-style partial-update
methods for embedded structs, which `stmt::patch` replaces.

## Motivation

**Tuple variants.** Rust enums carry unnamed fields like
`Phone(String, String)`. The schema parser rejects them, so users must
convert to struct variants with synthetic field names.

**Shared columns across variants.** When two variants carry the same
attribute — e.g. `Human` and `Animal` both have a `name` — a per-variant
column layout duplicates storage and blocks cross-variant queries on that
attribute.

**Enum within-variant partial updates.** Changing one field of an enum
variant requires reading the row, rebuilding the full variant value, and
writing it back. For a `Phone { country, number }` where only `number`
changes, that's a read-modify-write with a concurrency window.

**`.with_<field>` cleanup.** Embedded struct partial updates shipped as
closure methods (`.with_address(|a| a.city("Seattle"))`). `stmt::patch` and
`stmt::apply` cover the same cases across scalars, embedded structs, and
enums, so `.with_<field>` duplicates them.

## User-facing API

### Tuple variants

A tuple variant maps each unnamed field to its own column. The default column
name is `{field}_{variant}_{index}`:

```rust
#[derive(toasty::Embed)]
enum Contact {
    #[column(variant = 1)]
    Phone(String, String),
}
// Columns: contact, contact_phone_0, contact_phone_1
```

Override with `#[column("name")]` on individual fields:

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

### Shared columns across variants

Multiple variants share a column by giving the same `#[column("name")]` to a
field in each variant:

```rust
#[derive(Model)]
struct Character {
    #[key]
    #[auto]
    id: u64,
    creature: Creature,
}

#[derive(toasty::Embed)]
enum Creature {
    #[column(variant = 1)]
    Human {
        #[column("name")] name: String,
        profession: String,
    },
    #[column(variant = 2)]
    Animal {
        #[column("name")] name: String,
        species: String,
    },
}
// Columns:
//   creature                    (discriminator)
//   creature_name               (shared)
//   creature_human_profession
//   creature_animal_species
```

Filter against the shared column directly for cross-variant queries, or use
`.matches()` for variant-specific filters:

```rust
// Cross-variant: any creature named "Bob"
Character::all().filter(
    Character::FIELDS.creature().name().eq("Bob")
);

// Variant-specific: humans named "Alice"
Character::all().filter(
    Character::FIELDS.creature().matches(
        Creature::VARIANTS.human().name().eq("Alice")
    )
);
```

### Partial updates via `stmt::patch`

`stmt::patch` takes a typed field path and a value and produces an
`Assignment` that updates one sub-field. For embedded structs:

```rust
user.update()
    .address(stmt::patch(Address::fields().city(), "Seattle"))
    .exec(&mut db)
    .await?;
```

Multiple sub-field updates compose with `stmt::apply`:

```rust
user.update()
    .address(stmt::apply([
        stmt::patch(Address::fields().street(), "456 Oak Ave"),
        stmt::patch(Address::fields().zip(), "97202"),
    ]))
    .exec(&mut db)
    .await?;
```

Nested patches pass an inner `stmt::patch` as the value of an outer one:

```rust
company.update()
    .headquarters(stmt::patch(
        Office::fields().address(),
        stmt::patch(Address::fields().street(), "456 Oak Ave"),
    ))
    .exec(&mut db)
    .await?;
```

For enums, a variant+field path updates one field of a specific variant and
leaves the discriminator unchanged:

```rust
#[derive(toasty::Embed)]
enum ContactMethod {
    #[column(variant = 1)]
    Email { address: String },
    #[column(variant = 2)]
    Phone { country: String, number: String },
}

user.update()
    .contact(stmt::patch(
        ContactMethod::variants().phone().number(),
        "555-1234",
    ))
    .exec(&mut db)
    .await?;
```

**Migration from `.with_<field>`.** Single-field closure calls map
one-to-one to `stmt::patch`:

```rust
// Before
user.update().with_address(|a| a.city("Seattle")).exec(&mut db).await?;

// After
user.update()
    .address(stmt::patch(Address::fields().city(), "Seattle"))
    .exec(&mut db).await?;
```

Multi-field calls become `stmt::apply`:

```rust
// Before
user.update().with_address(|a| a.street("456 Oak Ave").zip("97202")).exec(&mut db).await?;

// After
user.update()
    .address(stmt::apply([
        stmt::patch(Address::fields().street(), "456 Oak Ave"),
        stmt::patch(Address::fields().zip(), "97202"),
    ]))
    .exec(&mut db).await?;
```

## Behavior

**Tuple variants.** Each unnamed field becomes a nullable column. The column
holds the field's value when the row's discriminator matches the variant and
NULL otherwise. Non-newtype tuple structs outside of enums stay rejected.

**Shared columns.** Fields that share a column must have compatible types,
checked at schema build time — same primitive type, or types with a defined
conversion. The shared column is nullable, since only variants that use it
write a value.

**Enum within-variant patches.** A `stmt::patch` on a variant+field path
applies only to rows whose current discriminator matches the variant named
in the path. Rows with a different discriminator pass through unchanged.
Switching variants requires full replacement via
`.field(FullVariant { ... })`. A `stmt::patch` never writes the
discriminator column.

**`.with_<field>` removal.** The update builder stops generating
`.with_<field>` for embedded types. Existing callers fail to compile and
switch to `stmt::patch`.

## Edge cases

- Shared-column types that differ in width (e.g. `i32` in one variant,
  `i64` in another) are rejected at schema build time, not coerced.
- `stmt::patch` on a row whose variant differs from the path's variant is
  a no-op for that row. A bulk update updates only matching rows.
- A shared column holds NULL when no variant writes it. Readers handle
  NULL even if every defined variant writes the column.
- Tuple variants with zero fields collapse to the unit-variant case.

## Driver integration

The three extensions reuse existing encoding paths:

- Tuple variants add more columns, encoded per-column like struct-variant
  fields.
- Shared columns reuse an existing column; drivers see the same column
  shape.
- `stmt::patch` compiles to a `Set` assignment at a longer projection. SQL
  drivers emit `UPDATE ... SET col = ...`; DynamoDB uses its existing
  nested-attribute update path. Variant-conditional assignment compiles
  to a `CASE` expression (SQL) or a conditional `UpdateExpression`
  (DynamoDB) on top of the existing path.

Drivers receive no new `Operation` variants.

## Alternatives considered

**Per-variant update builder.** The earlier design exposed enum partial
updates through nested closure builders:
`.with_contact(|c| c.phone(|p| p.with_number(|n| n.set("..."))))`. Each
nesting level added a generated builder type that duplicated the `fields()`
path infrastructure. `stmt::patch` reuses the typed-path accessors and
composes with scalars and has-many relations without generating additional
builder types.

**JSON-serialized tuple variants.** Serializing the whole variant into a
single JSON column avoids adding columns but blocks per-field indexes and
filters. Only appropriate for variants that are never queried by sub-field.

## Open questions

- Which primitive coercions between shared-column types are allowed
  silently (e.g. `i32` → `i64` widening) and which require an explicit
  `#[column(type = ...)]` override?
- Does DynamoDB's `UpdateExpression` path compose with a discriminator
  check for variant-conditional `stmt::patch`, or does it need a new
  condition-expression helper?

## Out of scope

- Non-newtype tuple structs outside of enums. `#[derive(Embed)]` on a
  tuple struct has no column-naming story; users convert to a named
  struct.
- Switching variants through partial updates. `stmt::patch` updates
  fields within the current variant; a variant change uses full
  replacement.
- DynamoDB-specific index shapes for tuple variants and shared columns.
  Both reuse existing per-column encoding, so existing GSI rules apply.
