# Data-Carrying Enum Implementation Design

Builds on unit enum support (#355). See `docs/design/enums-and-embedded-structs.md`
for the user-facing design.

## Value Stream Encoding

Unit and data variants are encoded differently in the value stream:

- **Unit variant**: `Value::I64(discriminant)` — unchanged from unit enum encoding
- **Data variant**: `Value::Record([I64(discriminant), ...active_field_values])`

Only the active variant's fields appear in the record; inactive variant columns (NULL
in the DB) are not included. `Primitive::load` dispatches on the value type:

```
I64(d)      => unit variant with discriminant d
Record(r)   => data variant; r[0] is the discriminant, r[1..] are fields
```

## Schema Changes

`EnumVariant` gains a `fields: Vec<Field>` — the same `Field` type used by
`EmbeddedStruct`. Field indices are assigned globally across all variants within the
enum, keeping `FieldId { model: enum_id, index }` as a unique identifier consistent
with how `EmbeddedStruct` works. The `primary_key`, `auto`, and `constraints`
members of `Field` are always `false`/`None`/`[]` for variant fields.

`Primitive::ty()` changes based on variant content:
- Unit-only enum → `Type::I64` (unchanged)
- Any data variant present → `Type::Model(Self::id())`, same as embedded structs

## Codegen Changes

**Parsing**: `toasty-codegen/src/schema/` must parse variant fields and include them
in `EmbeddedEnum` registration so the runtime schema is complete.

**`Primitive::load`**: generated arms dispatch on value type first (I64 vs Record),
then on the discriminant within each branch. Data variant arms load each field from
its positional index in the record.

**`IntoExpr`**: unit variants emit `Value::I64(disc)` as today; data variants emit
`Value::Record([I64(disc), field_exprs...])`.

## Engine: `Expr::Match`

Both `table_to_model` and `model_to_table` are expressed using a new AST node:

```
Match { subject: Expr, arms: [(pattern: Value, expr: Expr)] }
```

`Expr::Match` is never serialized to SQL — it is either evaluated in the engine
(for writes) or eliminated by the simplifier before the plan stage (for reads/queries).

### table_to_model

For an enum field, `table_to_model` emits a `Match` on the discriminator column.
Each arm produces the value shape `Primitive::load` expects: unit arms emit
`I64(disc)`, data arms emit `Record([I64(disc), ...field_col_refs])`.

### model_to_table

Runs the inverse: the incoming value (`I64` or `Record`) is matched on its
discriminant, and each arm emits a flat record of all enum columns in DB order —
setting the discriminator and active variant fields, and `NULL`ing every inactive
variant column. This NULL-out is mandatory: because writes may not have a loaded
model, the engine has no knowledge of the prior variant and must clear all
non-active columns unconditionally.

## Queries

The query builder generates field references and projections as it does for embedded
structs. After lowering, `ref(enum_field)` is replaced by the `table_to_model`
`Expr::Match`. The simplifier then eliminates the `Match` using **case distribution**:

> `f(Match(subj, arms))` → `Match(subj, [p => f(e) for (p, e) in arms])`

Push `f` into each arm, fold constants, discard arms that become `false`, then
rewrite the residual `Match(subj, arms)` as `OR(subj = pi AND ci)`.

**Variant-only check** (`is_email()`): pushing `.eq(I64(1))` into the arms produces
`true` for the matching arm and `false` for all others (type mismatch between `I64`
and `Record`). Result: `disc_col = 1`.

**Variant+field check** (`email.address.eq("x")`): pushing `project([1])` into the
arms first eliminates unit arms (scalar has no `[1]`) and extracts the field column
from data arms. Pushing `.eq("x")` after that produces `disc_col = N AND field_col =
"x"`. The discriminant guard falls out automatically — it is a consequence of case
distribution, not a special case.

Case distribution is preferable to a flat-Record `table_to_model` (all variant
columns always present, with NULLs) because the encoding is semantically precise,
the rule is general, and the resulting SQL is exact.

### OR Tautology Elimination

`is_bar() || is_baz()` over an enum with exactly `{Bar, Baz}` is a tautology.
After lowering and case distribution, this becomes `disc_col = 1 OR disc_col = 2`.
`simplify_expr_or` detects that the value set covers all discriminants for the enum
type and replaces the expression with `TRUE` (non-nullable field) or `IS NOT NULL`
(nullable). This fires pre-lower, where the enum field type is directly accessible in
the schema. For data-carrying enums, case distribution first normalises each
`is_variant()` check to a discriminant comparison; the tautology rule then fires
independently.

## Partial Updates

See `docs/design/enums-and-embedded-structs.md § Enum updates` for the public API.

**Whole-variant replacement** (`.contact(value)`): `IntoExpr` encodes the value as
`I64` or `Record`; the engine expands it through the `model_to_table` `Expr::Match`,
producing flat column assignments that set the discriminator and active variant
fields, and NULL every inactive column.

**Within-variant partial update** (`.with_contact(|c| c.phone(...))`): the update
builder emits assignments for the specific columns directly — no discriminant
dispatch. Because the target variant is statically known (the caller invoked
`.phone(...)` not `.email(...)`), the engine automatically injects a condition
`disc_col = N` into the WHERE clause. This turns a mismatched-variant write into a
no-op (zero rows affected) rather than silent corruption.

The codegen produces `{Enum}Update<'a>` with one method per variant, each scoped to
that variant's fields. Calling a unit variant's method (no fields) sets only the
discriminator, equivalent to whole-variant replacement.

## Correctness Sharp Edges

**Whole-variant replacement must NULL all inactive columns.** The engine has no
knowledge of the prior variant for query-based updates, so the `model_to_table` arms
unconditionally NULL every column they do not own.

**NULL discriminators are disallowed.** The discriminator column carries `NOT NULL`,
consistent with unit enums today. `Option<Enum>` support is a future concern.

**Within-variant partial updates on mismatched variants affect zero rows.** The
auto-injected discriminant condition prevents corruption but produces a silent no-op
if the DB holds a different variant. Future rows-affected checking could surface this
as an error.

**No DB-level integrity for active variant fields.** All variant columns are nullable
(to accommodate inactive variants), so a NULL in a required active field is caught
only at load time by `Primitive::load`, not at write time.

**Unknown discriminants fail at load time.** An unrecognized discriminant (e.g. from
a newer schema version) produces a runtime error. Removing a variant requires a data
migration.

## DynamoDB

Equivalent encoding to be determined when implementing the DynamoDB driver phase.

## Implementation Phases

1. **Schema**: add `fields: Vec<Field>` to `EnumVariant`; update `Register::schema()`
   codegen; `Primitive::ty()` returns `Type::Model` when any data variant is present.

2. **Value encoding**: update `Primitive::load()` codegen for unit/data dispatch;
   update `IntoExpr` to emit `Record` for data variants.

3. **`Expr::Match`**: add `Match`/`MatchArm` to `toasty-core::stmt`; implement
   `table_to_model` and `model_to_table` for enum fields.

4. **Simplifier**: case distribution in `simplify_expr_binary_op` and
   `simplify_expr_project`; residual-Match → `OR(subj = pi AND ci)` rewrite;
   OR tautology elimination in `simplify_expr_or`.

5. **Integration tests**: CRUD for pure data-carrying enum; mixed enum; nested
   enum-in-struct and struct-in-enum; filter by variant; filter by variant field;
   whole-variant and within-variant update.

6. **DynamoDB**: equivalent encoding in the DynamoDB driver.

## Open Questions

- **`SparseRecord` / `reload`**: within-variant partial updates are supported, so
  `SparseRecord` and `reload` are needed for enum variant fields. Determine how
  `reload` should handle a `SparseRecord` scoped to a specific variant's fields —
  the in-memory model must update only the changed fields without disturbing the
  discriminant or other variant columns.

- **Shared columns**: variants sharing a column via `#[column("name")]` is in the
  user-facing design. Schema parsing should record shared columns in Phase 1; full
  query support is a follow-on.
