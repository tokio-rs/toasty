# Enum Projection Design

Projections are used in two distinct roles in Toasty:

1. **Schema navigation** — identifying a field by its global index within a model
   (`FieldId { model, index }`, used to look up a field's mapping).
2. **Runtime traversal** — navigating into a `Value::Record` or `Expr::Record`
   by positional index.

For embedded structs these two roles are the same number. For data-carrying enum
variant fields they are not, and that divergence is what this doc analyses.

---

## Concrete example

```rust
#[derive(Embed)]
enum Event {
    #[column(variant = 1)]
    Login { user_id: String, ip: String },
    #[column(variant = 2)]
    Purchase { item_id: String, amount: i64 },
}
```

### Global field indices (app schema)

Field indices are assigned globally across all variants:

| Field             | Global index |
|-------------------|:------------:|
| `Login.user_id`   | 0            |
| `Login.ip`        | 1            |
| `Purchase.item_id`| 2            |
| `Purchase.amount` | 3            |

### Value encoding

`Primitive::load` / `IntoExpr` use a compact, per-variant record:

```
Event::Login    { user_id, ip }     → Record([I64(1), user_id_val, ip_val])
Event::Purchase { item_id, amount } → Record([I64(2), item_id_val, amount_val])
```

Position 0 is always the discriminant; positions 1… are the active variant's
fields in declaration order (local indices, not global).

### Expression encoding (table_to_model)

`build_table_to_model_field_enum` mirrors the value layout:

```
Match(disc_col, [
  arm(1) → Record([disc_col, user_id_col, ip_col]),
  arm(2) → Record([disc_col, item_id_col, amount_col]),
], null)
```

### Sub-projections stored in the mapping

`map_field_enum` registers each variant field with its **local record position**
as the `sub_projection`:

| Field             | Global index | sub_projection (local record pos) |
|-------------------|:------------:|:---------------------------------:|
| `Login.user_id`   | 0            | `[1]`                             |
| `Login.ip`        | 1            | `[2]`                             |
| `Purchase.item_id`| 2            | `[1]`                             |
| `Purchase.amount` | 3            | `[2]`                             |

Two observations:

- `sub_projection` and global field index are **different numbers** for every
  variant field (global 0 → local `[1]`, global 1 → local `[2]`, etc.).
- Two fields from different variants share the same `sub_projection`:
  `Login.user_id` and `Purchase.item_id` both map to `[1]`; `Login.ip` and
  `Purchase.amount` both map to `[2]`.

---

## Where the two index spaces are used

### Value traversal

`Value::entry(path)` and `value.project(projection)` index directly into the
`Record` by position. The correct traversal for `Login.ip` uses `[2]`, not `[0]`
or `[1]`.

```rust
let v = Event::Login { user_id: "alice".into(), ip: "1.2.3.4".into() };
// encoded as Record([I64(1), "alice", "1.2.3.4"])
v.project([2])  // → "1.2.3.4"  (local position [2])
v.project([1])  // → "alice"    (local position [1])
v.project([0])  // → I64(1)     (discriminant, not a field!)
```

### Expression traversal via the simplifier

`expr_project.rs` handles `Project(Record([...]), [i])` by indexing into the
`Expr::Record` at position `i`. For the `table_to_model` Match arms this means
the same local positions:

```
Project(Record([disc_col, user_id_col, ip_col]), [2]) → ip_col   ✓
Project(Record([disc_col, user_id_col, ip_col]), [1]) → user_id_col  ✓
Project(Record([disc_col, user_id_col, ip_col]), [0]) → disc_col  (not a field)
```

So the **local record position** is correct for both value and expression
traversal. They are consistent with each other, which is good.

The problem is that the **global field index is not the same as the local record
position**, so code that builds a projection using a global field index and then
tries to traverse the encoded value (or table_to_model expression) with it will
get the wrong result.

---

## The overloading tension

The `Projection` type is used for both roles without distinguishing them:

- **Schema navigation**: code calls `resolve_field_mapping(&projection)` where
  the projection steps are global model field indices. `build_update_returning`
  emits `project(ref_self_field(root_field_id), sub_projection)` where
  `sub_projection` is the **local record position** from the mapping — correct
  for later value/expression traversal.

- **Path building** (`path.rs`): the query builder constructs paths from global
  field indices (`FieldId.index`). For embedded structs this works because the
  struct's field index equals its local record position. For enum variant fields
  this breaks: `FieldId { index: 2 }` for `Purchase.item_id` becomes a
  projection step `[2]`, but the correct record position is `[1]`.

For embedded structs the two roles happen to use the same numbers, so there is
no bug today. The divergence only surfaces for enum variant fields.

---

## Consequences for filter query distribution (Phase 4)

When the simplifier distributes `Project(Match(disc_col, arms), [pos])` into
the arms, each arm's `Record` gets projected at `[pos]`. Since different variants
can hold different logical fields at the same local position, the result is not
always semantically correct.

### Case A: whole-enum equality (safe)

```
Match(disc_col, [arm(1, Record([disc, user_id, ip])),
                 arm(2, Record([disc, item_id, amount]))]) = Record([I64(1), "alice", "1.2.3.4"])
```

After BinaryOp distribution over Match, each arm gets `arm_expr = RHS`:

```
(disc = 1 AND Record([disc, user_id, ip]) = Record([I64(1), "alice", "1.2.3.4"]))
OR
(disc = 2 AND Record([disc, item_id, amount]) = Record([I64(1), "alice", "1.2.3.4"]))
```

Record decomposition on the arm-2 comparison yields `disc = 1` as one of the
sub-conditions, which contradicts `disc = 2`. That arm is eliminated. The
discriminant guard falls out automatically — **whole-enum equality is safe**.

### Case B: variant field projection (unsafe)

Suppose the query builder generates `Project(ref(event), [2])` to access
`Login.ip` (using global field index 1 — but that's the wrong number; say it
correctly generates `[2]` as the local position). Distribution gives:

```
Project(Match(disc, [arm(1, Record([disc, uid, ip])), arm(2, Record([disc, iid, amt]))]), [2])
→ Match(disc, [arm(1, ip_col), arm(2, amount_col)])
→ (disc = 1 AND ip_col)  OR  (disc = 2 AND amount_col)
```

The arm-2 result (`disc = 2 AND amount_col = "x"`) is semantically wrong: a
filter for Login.ip should never touch Purchase rows. If a Purchase row's
`amount` column happens to match the string "x", it will be returned
incorrectly.

This is the core unsafeness: **two different fields share the same local record
position across variants**, so distributing a projection through all arms gives
the wrong field for non-matching variants.

---

## Proposed fix: typed `Step` enum for `Projection`

Rather than treating all projection steps as plain `usize` indices, extend
`Projection` so each step carries its kind:

```rust
enum Step {
    Field(usize),    // positional access into a Record / struct
    Index(usize),    // positional access into a List
    Variant(usize),  // discriminant-guarded access into an enum
}
```

### Discriminant vs. variant index

`Step::Variant(n: usize)` carries a **variant index** — the 0-based position of
the variant in the variants list — not the raw DB discriminant value.

This distinction matters because discriminants are a DB-layer concern. Today they
are always `i64`, but the schema design allows arbitrary types (e.g.
`String("email")`). Carrying a raw discriminant value in the `Step` would force
`Variant` to hold a `Value` or become generic, and would entangle the app layer
with whatever storage type the DB happens to use.

By using variant index instead, `Variant(n)` is always a `usize` and the app
layer is completely decoupled from the DB discriminant type. The DB discriminant
is an opaque storage detail.

### Normalization in lowering

The translation from DB discriminant → variant index happens once, in the
`table_to_model` expression built during schema lowering. Concretely, the current
`Match(disc_col, [arm(I64(1), ...), arm(I64(2), ...)])` gets an additional
normalization step so that the arm patterns become variant indices:

```
Match(normalize(disc_col), [arm(0, Record([norm, uid_col, ip_col])),
                             arm(1, Record([norm, iid_col, amt_col]))])
```

where `normalize` maps each DB discriminant value to its variant index
(`I64(1) → I64(0)`, `I64(2) → I64(1)`, or `String("email") → I64(0)`, etc.).

Everything above this normalization — value encoding, projections,
`Primitive::load`, the simplifier — works exclusively with variant indices and
never sees raw discriminant values.

### What `Variant(n)` does at each layer

**Value traversal** — values at the app layer carry a normalized variant index at
position 0. `Record([I64(0), "alice", "1.2.3.4"]).entry([Variant(0), Field(2)])`:
1. `Variant(0)`: check `record[0] == I64(0)` → pass through
2. `Field(2)`: `record[2]` → `"1.2.3.4"` ✓

`Record([I64(1), "item", 100]).entry([Variant(0), Field(2)])`:
1. `Variant(0)`: variant index = 1, no match → `null` ✓

**Expression traversal / simplifier** — arms are in variant-index order, so
`Variant(n)` selects `arms[n]` positionally (no pattern search needed):

```
Project(Match(subj, arms, else), [Variant(n), ..rest]) →
    Project(arms[n].expr, rest)
```

This is a pure positional lookup. `arms[n]` is always the correct arm because the
`table_to_model` builder emits arms in variant-declaration order.

### Effect on `sub_projection`

The collision in the current mapping disappears, and every path is unambiguous:

| Field             | Old `sub_projection` | New `sub_projection`       |
|-------------------|----------------------|----------------------------|
| `Login.user_id`   | `[1]`                | `[Variant(0), Field(1)]`   |
| `Login.ip`        | `[2]`                | `[Variant(0), Field(2)]`   |
| `Purchase.item_id`| `[1]` ← collision    | `[Variant(1), Field(1)]`   |
| `Purchase.amount` | `[2]` ← collision    | `[Variant(1), Field(2)]`   |

### The semantic shift

`Field(n)` and `Index(n)` are **total** steps (always produce a value).
`Variant(n)` is **partial** — it only applies when the variant index matches,
returning `null` otherwise. In optics terms, `Field`/`Index` are lenses;
`Variant` is a prism. This is a more honest description of what enum access
actually is.

### Implementation cost

`Projection` is currently backed by `Vec<usize>` and `Deref`s to `&[usize]`,
with `PartialEq<usize>`, `Equivalent<Projection> for usize`, `From<[usize; N]>`,
and `Hash` compatible with raw `usize` keys in `IndexMap`. All of this breaks
when the element type changes to `Step`.

Callsites that would need updating include: `EntryPath` / `entry()` on `Value`
and `Expr`, `simplify_expr_project`, `build_pk_lowering`'s `map_projections`,
`resolve_field_mapping`, `AssignmentInput::resolve_ref`, all `sub_projection`
construction in `table.rs`, path building in codegen's `path.rs`, and
`IndexMap<ColumnId, usize>` lookups in `FieldStruct`.

The `table_to_model` builder also needs to emit the discriminant normalization
expression and switch arm patterns to variant indices.

### Status

Tracked as the correct long-term fix. Must be attempted before variant-field
query support (Phase 4) can be implemented cleanly. Whole-enum equality filters
(Case A above) are safe with the current plain-`usize` projection and can ship
without this change; variant-field filters require it.

## Summary

| Context | Index used today | Correct? |
|---------|-----------------|----------|
| `Value::entry(proj)` traversal | local record position (raw discriminant at [0]) | ✓ for current code; needs variant-index normalization |
| `Expr::Record` entry in simplifier | local record position | ✓ (if proj is local) |
| `sub_projection` in mapping | local record position | ✗ ambiguous across variants |
| `path.rs` query builder steps | global field index | ✗ for enum variant fields |
| `resolve_field_mapping` schema walk | global field index | ✓ (schema lookup, not traversal) |

The invariant to enforce with the typed `Step` design: **everything above the
`table_to_model` normalization step works with variant indices (`usize`), never
with raw DB discriminant values**. The normalization in lowering is the single
point where DB discriminant type is handled; all projection traversal
(`Field`/`Variant`/`Index` steps) is decoupled from it.
