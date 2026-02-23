# Embedded Enum Implementation Plan

Implements the enum portion of `docs/design/enums-and-embedded-structs.md`.

## Phase 1: Unit-Only Enums

```rust
#[derive(toasty::Embed)]
enum Status {
    #[column(variant = 1)] Pending,
    #[column(variant = 2)] Active,
    #[column(variant = 3)] Done,
}
// Stored as: status INTEGER NOT NULL
```

### Value Representation

Unit enums (all variants carry no data) are represented **directly as the discriminant
value** — no `Record` wrapper. This avoids a heap allocation for the most common case.
Data-carrying enums (Phase 2) will use `Value::Record([I64(discriminant), ...fields])`.

The discriminant is the user-specified `#[column(variant = N)]` value, stored identically
at app level and in the DB column — no conversion needed.

- `Status::Pending` → `Value::I64(1)`
- `Status::Active`  → `Value::I64(2)`
- DB column stores `1` or `2` directly

### Mapping

Because the value IS the column value, no conversion expression is needed:

- `model_to_table[col]`: enum field value → column value (direct)
- `table_to_model[field]`: column value → enum field value (direct)

Filter lowering (e.g. `user.status == Status::Active`):
- App expr: `user.status == Value::I64(2)` — already the DB value, no lowering needed

No new `Expr` variants are needed for the discriminant mapping.

### Remaining Changes

**`toasty-core` schema builder (`builder/table.rs`)**
- `populate_columns` (`FieldTy::Embedded` arm, line ~229): detect when embedded target is
  `EmbeddedEnum` and create a single INTEGER column instead of recursing (currently recursion
  into `EmbeddedEnum` produces no columns — the `&[]` early-return only guards the recursive
  call; the column creation itself is missing)
- `map_fields_recursive` (`FieldTy::Embedded` arm, line ~462): detect when embedded target
  is `EmbeddedEnum` and build a `mapping::Field::Primitive` with direct identity expressions
  instead of calling `expect_embedded_struct()` (currently panics on enum)

**`toasty-codegen`**
- `generate_embed()`: try `ItemEnum` if `ItemStruct` parse fails
- New enum path: validate unit variants, parse `#[column(variant = N)]`, generate `Register` + `Embed` + `Primitive` + `IntoExpr` impls
- `schema()` returns `Model::EmbeddedEnum(EmbeddedEnum { ... })` (no `fields`)
- `ty()` returns `Type::I64` (unit enums are just an integer at the value level)
- `field_ty()` returns `FieldTy::Embedded` (same dispatch as structs)
- `load(Value::I64(d))`: match on `d` against discriminant values to construct Rust variant
- `into_expr()`: produce `Value::I64(discriminant)` using the `#[column(variant = N)]` value
- Generate a thin `StatusField { path: Path<Status> }` accessor with `eq()`/`ne()`; `UpdateBuilder = ()` (no partial updates)

**No changes to:** drivers, `toasty-sql`, `toasty-macros`

### Tests

Add to `tests/tests/`: create model with unit enum field, filter by variant, update field.

## Phase 2: Data-Carrying Enums

```rust
#[derive(toasty::Embed)]
enum ContactMethod {
    #[column(variant = 1)] Email { address: String },
    #[column(variant = 2)] Phone { country: String, number: String },
}
// contact INTEGER NOT NULL
// contact_email_address TEXT
// contact_phone_country TEXT
// contact_phone_number  TEXT
```

Significantly larger — touches `toasty-core`, `toasty-codegen`, and the engine.
Defer until Phase 1 is working.

Key additions over Phase 1:
- `Model::EmbeddedEnum` variant fields (non-empty `EmbeddedEnum::variants[N].fields`)
- Data-carrying enums use `Value::Record([I64(discriminant), ...fields])` — engine and
  codegen must handle both the direct `Value::I64` (unit) and `Value::Record` (data) forms
- `populate_columns`: create nullable variant field columns with prefix `{field}_{variant_name}`
- `mapping::Field::Enum(FieldEnum)` with per-variant column maps and NULL-out logic for writes
- Engine lower/simplify: enum field access expressions, NULL-out inactive variant columns on write
- Codegen: `{Type}Fields` accessor struct, `{Type}Update<'a>` builder
