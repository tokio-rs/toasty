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

**Parsing**: `toasty-codegen/src/schema/` parses variant fields and includes them
in `EmbeddedEnum` registration so the runtime schema is complete.

**`Primitive::load`**: generated arms dispatch on value type first (I64 vs Record),
then on the discriminant within each branch. Data variant arms load each field from
its positional index in the record.

**`IntoExpr`**: unit variants emit `Value::I64(disc)` as today; data variants emit
`Value::Record([I64(disc), field_exprs...])`.

**`{Enum}Fields` struct**: all enums (unit-only and data-carrying) generate a
`{Enum}Fields` struct with `is_{variant}()` methods for discriminant-only filtering.
For data-carrying enums, `is_{variant}()` uses `project(path, [0])` to extract the
discriminant from the record representation. For unit-only enums, it compares the
path directly. The struct also delegates comparison methods (`eq`, `ne`, etc.) to
`Path<Self>`.

## Engine: `Expr::Match`

Both `table_to_model` and `model_to_table` are expressed using:

```
Match { subject: Expr, arms: [(pattern: Value, expr: Expr)], else_expr: Expr }
```

`Expr::Match` is never serialized to SQL — it is either evaluated in the engine
(for writes) or eliminated by the simplifier before the plan stage (for reads/queries).

### table_to_model

For an enum field, `table_to_model` emits a `Match` on the discriminator column.
Each arm produces the value shape `Primitive::load` expects: unit arms emit
`I64(disc)`, data arms emit `Record([I64(disc), ...field_col_refs])`.

### else branch: `Expr::Error`

The else branch of an enum `Match` represents the case where the discriminant column
holds an unrecognized value — semantically unreachable for well-formed data.

For data-carrying enums, the else branch is `Record([disc_col, Error, ...Error])` —
the same Record shape as data arms, but with `Expr::Error` in every field slot. This
design is critical for the simplifier: projections distribute uniformly into the else
branch, and field-slot projections yield `Expr::Error` (correct: accessing a field
on an unknown variant is an error), while discriminant projections (`[0]`) yield
`disc_col` (the same as every arm). This enables the uniform-arms optimization to
fire after projection.

For unit-only enums with data variants, else is `Expr::Error` directly.

### model_to_table

Runs the inverse: the incoming value (`I64` or `Record`) is matched on its
discriminant, and each arm emits a flat record of all enum columns in DB order —
setting the discriminator and active variant fields, and `NULL`ing every inactive
variant column. This NULL-out is mandatory: because writes may not have a loaded
model, the engine has no knowledge of the prior variant and must clear all
non-active columns unconditionally.

## Simplifier Rules

### Project into Match (expr_project.rs)

Distributes a projection into each Match arm AND the else branch:

```
project(Match(subj, [p => e, ...], else), [i])
  → Match(subj, [p => project(e, [i]), ...], else: project(else, [i]))
```

Projection is pushed into the else branch unconditionally — `Expr::Error` inside
a Record is handled naturally (projecting `[0]` out of `Record([disc, Error])`
yields `disc`; projecting `[1]` yields `Error`).

### Uniform arms (expr_match.rs)

When all arms AND the else branch produce the same expression, the Match is
redundant:

```
Match(subj, [1 => disc, 2 => disc], else: disc)  →  disc
```

The else branch MUST equal the common arm expression for this rule to fire. This
makes the transformation provably correct — no branch is dropped that could produce
a different value.

### Match elimination in binary ops (expr_binary_op.rs)

Distributes a binary op over match arms, producing an OR of guarded comparisons.
The else branch is included with a negated guard:

```
Match(subj, [p1 => e1, p2 => e2], else: e3) == rhs
  → OR(subj == p1 AND e1 == rhs,
       subj == p2 AND e2 == rhs,
       subj != p1 AND subj != p2 AND e3 == rhs)
```

Each term is fully simplified inline. Terms that fold to false/null are pruned.
No special handling is needed for the else branch — it is always included and
existing simplification rules handle `Expr::Error` naturally (see below).

### `Expr::Error` semantics

`Expr::Error` is treated as "unreachable" — not as a poison value that propagates.
No special Error propagation rules exist. Instead, existing rules eliminate Error
through the surrounding context:

- **Data-carrying enum else**: `Record([disc, Error, ...])`. After tuple
  decomposition, the guard `disc != p1 AND disc != p2` contradicts the
  decomposed `disc == c` from the comparison target. The contradicting
  equality rule (`a == c AND a != c → false`) folds the AND to false.

- **`false AND (Error == x)`**: The `false` short-circuit in AND eliminates the
  term without needing to simplify `Error == x`.

- **`Record([1, Error]) == Record([0, "alice"])`**: Tuple decomposition produces
  `1 == 0 AND Error == "alice"`. The `1 == 0 → false` folds the AND to false.

In all well-formed cases, the guard constraints around Error cause the branch to
be pruned without requiring Error-specific rules.

### Type inference for `Expr::Error`

`Expr::Error` infers as `Type::Unknown`. `TypeUnion::insert` skips `Unknown`, so
an Error branch in a Match doesn't widen the inferred type union.

### Variant-only filter flow

`is_email()` generates `eq(project(path, [0]), I64(1))`. After lowering:

```
eq(project(Match(disc, [1 => Record([disc, addr]), 2 => Record([disc, num])],
                 else: Record([disc, Error])), [0]),
   I64(1))
```

1. Project-into-Match distributes `[0]` into all branches including else
2. `project(Record([disc, addr]), [0])` → `disc` (for each arm)
3. `project(Record([disc, Error]), [0])` → `disc` (for else)
4. Uniform-arms fires: all arms AND else produce `disc` → folds to `disc`
5. Result: `eq(disc, I64(1))` — a clean `disc_col = 1` predicate

### Full-value equality filter flow

`contact().eq(ContactInfo::Email { address: "alice@example.com" })` generates
`eq(path, Record([I64(1), "alice@example.com"]))`. After lowering:

```
eq(Match(disc, [1 => Record([disc, addr]), 2 => Record([disc, num])],
         else: Record([disc, Error])),
   Record([I64(1), "alice@example.com"]))
```

1. Match elimination distributes eq into each arm AND else as OR
2. `disc == 1 AND Record([disc, addr]) == Record([I64(1), "alice"])` → simplifies
3. `disc == 2 AND Record([disc, num]) == Record([I64(1), "alice"])` → false (pruned)
4. Else: `disc != 1 AND disc != 2 AND Record([disc, Error]) == Record([I64(1), "alice"])`
   → tuple decomposition: `disc != 1 AND disc != 2 AND disc == 1 AND Error == "alice"`
   → contradicting equality (`disc == 1 AND disc != 1`) → false (pruned)
5. Result: `disc_col = 1 AND addr_col = 'alice@example.com'`

## Correctness Sharp Edges

**Whole-variant replacement must NULL all inactive columns.** The engine has no
knowledge of the prior variant for query-based updates, so the `model_to_table` arms
unconditionally NULL every column they do not own.

**NULL discriminators are disallowed.** The discriminator column carries `NOT NULL`,
consistent with unit enums today. `Option<Enum>` support is a future concern.

**Unknown discriminants fail at load time.** An unrecognized discriminant (e.g. from
a newer schema version) produces a runtime error via `Expr::Error`. Removing a
variant requires a data migration.

**No DB-level integrity for active variant fields.** All variant columns are nullable
(to accommodate inactive variants), so a NULL in a required active field is caught
only at load time by `Primitive::load`, not at write time.

## DynamoDB

Equivalent encoding to be determined when implementing the DynamoDB driver phase.

## Implementation Status

### Completed

1. **Schema**: `fields: Vec<Field>` on `EnumVariant`; codegen parsing; `Primitive::ty()`
   returns `Type::Model` for data-carrying enums.

2. **Value encoding**: `Primitive::load()` dispatches on I64 vs Record;
   `IntoExpr` emits Record for data variants.

3. **`Expr::Match` + `Expr::Error`**: Match/MatchArm AST nodes with visitors, eval,
   and simplifier integration. `Expr::Error` for unreachable branches.
   `build_table_to_model_field_enum` uses `Record([disc, Error, ...])` for the
   else branch.

4. **Simplifier**: project-into-Match distribution; uniform-arms folding (with
   else-branch check); Match-to-OR elimination in binary ops; case distribution
   for binary ops with Match operands.

5. **`{Enum}Fields` codegen**: all enums generate a fields struct with
   `is_{variant}()` methods and delegated comparison methods.

6. **Integration tests**: CRUD for data-carrying enums; full-value equality filter;
   variant-only filter (`is_email()`); unit enum variant filter (`is_pending()`).

### Remaining

- **Variant+field filter** (`contact().email().address().eq("x")`): per-variant field
  accessors that project into the variant's data fields. Requires generating
  accessor methods on the fields struct for each variant's fields.

- **Partial updates**: within-variant partial update builder.

- **OR tautology elimination**: `is_bar() || is_baz()` over `{Bar, Baz}` → `TRUE`.

- **DynamoDB**: equivalent encoding in the DynamoDB driver.

## Open Questions

- **`SparseRecord` / `reload`**: within-variant partial updates are supported, so
  `SparseRecord` and `reload` are needed for enum variant fields. Determine how
  `reload` should handle a `SparseRecord` scoped to a specific variant's fields —
  the in-memory model must update only the changed fields without disturbing the
  discriminant or other variant columns.

- **Shared columns**: variants sharing a column via `#[column("name")]` is in the
  user-facing design. Schema parsing should record shared columns in Phase 1; full
  query support is a follow-on.
