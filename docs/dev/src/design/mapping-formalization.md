# Mapping Layer Formalization

## Problem

Toasty's mapping layer connects model-level fields to database-level columns.
A model field's type may differ from its storage type (e.g., `Timestamp` stored
as `i64` or `text`). The mapping must be a **bijection** — every model value
encodes to exactly one stored value and decodes back losslessly. The bijection
operates at the **record level**, not per-field: `n` model fields may map to `m`
database columns (e.g., multiple fields JSON-encoded into a single column).

The bijection alone is not sufficient. When lowering expressions (filters, ORDER
BY, arithmetic) to the database, we need to know whether a given **operator** can
be pushed through the encoding. This is the question of whether the encoding is a
**homomorphism** with respect to that operator:

- For arithmetic: `encode(a ⊕ b) = encode(a) ⊕' encode(b)`
- For comparisons: `a < b ⟺ encode(a) <' encode(b)`

If yes, the operator can be evaluated in storage space (efficient, index-friendly).
If no, the database must first decode to the model type (SQL CAST) or the
operation must be evaluated application-side.

These are two decoupled concerns:

1. **Bijection** — can we round-trip values? (required for correctness)
2. **Operator homomorphism** — which operators preserve semantics through the
   encoding? (determines what can be pushed to the DB)

A mapping with no homomorphic operators is still valid — you can store and
retrieve. You just can't push any filters or ordering to the database.

## Examples

### Timestamp as `i64` (epoch seconds)

```
encode(ts) = ts.epoch_seconds()
decode(n)  = Timestamp::from_epoch_seconds(n)
```

**Bijection:** ✓ — lossless round-trip.

**`==` homomorphic:** ✓ — `ts1 == ts2 ⟺ encode(ts1) == encode(ts2)`

**`<` homomorphic:** ✓ — `ts1 < ts2 ⟺ encode(ts1) < encode(ts2)`

Epoch seconds preserve temporal ordering under integer comparison, so range
queries (`<`, `>`, `BETWEEN`) can operate directly on the raw column.

**`+` homomorphic:** ✓ — `encode(ts + 234s) = encode(ts) + 234`

Integer addition over epoch seconds preserves timestamp arithmetic.

### Timestamp as `text` (ISO 8601)

```
encode(ts) = ts.to_iso8601()
decode(s)  = Timestamp::parse_iso8601(s)
```

**Bijection:** ✓ — lossless round-trip (assuming canonical formatting).

**`==` homomorphic:** ✓ — injective encoding preserves equality.

**`<` homomorphic:** fragile — lexicographic order matches temporal order only
for fixed-width UTC formats. Not generally safe.

**`+` homomorphic:** ✗ — `text + 234` is meaningless.

### String with case inversion

```
encode(s) = s.invert_case()    // "Hello" → "hELLO"
decode(s) = s.invert_case()    // "hELLO" → "Hello"
```

**Bijection:** ✓ — case inversion is its own inverse.

**`==` homomorphic:** ✓ — injective, so equality is preserved. Encode the
search term the same way and compare.

**`<` homomorphic:** ✗ — ordering is reversed between cases:

```
"ABC" < "abc"                   (A=65 < a=97)
encode("ABC") = "abc"
encode("abc") = "ABC"
"abc" > "ABC"                   — ordering reversed
```

A valid mapping, but useless for range queries in storage space.

## Bijection by Construction

For arbitrary functions, bijectivity is undecidable. Instead of detecting it, we
**construct** mappings from known-bijective primitives and composition rules that
preserve bijectivity. If a mapping is built entirely from these, it is guaranteed
valid.

### Composition rules

- **Sequential:** `f ∘ g` is a bijection if both `f` and `g` are.
- **Parallel/product:** `(f(a), g(b))` is a bijection if both `f` and `g` are.

These compose freely — complex mappings built from simple bijective pieces are
automatically valid. Homomorphism properties, however, may be lost at each
composition step and must be tracked separately.

### Dimensionality: multiple fields → one column

Two fields may map to the same column if and only if the model constrains them
to always hold the same value (an equivalence class). In this case no information
is lost and the mapping remains a bijection — but only over the restricted domain
where the constraint holds. Without such a constraint, collapsing two independent
fields into one column destroys injectivity.

This gives us **computed fields** as a natural consequence. Two fields can
reference the same column through different bijective transformations:

```
regular:  String → column              (identity)
inverted: String → invert_case(column) (bijection)
```

Because the transformations are bijections, both fields are readable AND writable.
Writing `regular = "Hello"` stores `"Hello"` in the column; `inverted`
automatically becomes `"hELLO"`. Writing `inverted = "hELLO"` applies the inverse
to store `"Hello"`; `regular` is automatically `"Hello"`. Data flow in both
directions is fully determined by the bijection — no special computed-field
machinery needed.

## Computed Fields

Storage is the source of truth. Each field is a **view** of the underlying
column(s) through its bijection. Computed fields are a direct consequence: when
multiple fields reference the same column through different bijections, each
field is a different view of the same stored data.

### Schema representation

Each field stores a bijection pair:
- `field_to_column`: encode — compute column value from field value (inverse)
- `column_to_field`: decode — compute field value from column value (forward)

A reverse index maps each column to the set of fields that reference it.

### Write propagation

When a field is set, the column value is determined, which determines all sibling
fields:

1. Compute column value: `col = field_a.field_to_column(new_value)`
2. For each sibling field on the same column:
   `field_b = field_b.column_to_field(col)`

The composed transform between two fields sharing a column is:
`field_b.column_to_field(field_a.field_to_column(value))`

### Conflict detection

If the user sets two fields that share a column in the same operation, the
resulting column values must agree. If
`field_a.field_to_column(val_a) ≠ field_b.field_to_column(val_b)`, the write is
invalid and must be rejected.

## Bijective Primitives

Three categories of bijective primitives, each with encode/decode halves:

### Type reinterpretation

Converts a single value between two types with the same information content.
Implemented as `Expr::Cast` in both directions.

Current pairs:
- Timestamp ↔ String (ISO 8601)
- Uuid ↔ String
- Uuid ↔ Bytes
- Date ↔ String
- Time ↔ String
- DateTime ↔ String
- Zoned ↔ String
- Timestamp ↔ DateTime
- Timestamp ↔ Zoned
- Zoned ↔ DateTime
- Decimal ↔ String
- BigDecimal ↔ String
- Integer widening/narrowing (i8 ↔ i16 ↔ i32 ↔ i64, etc.)

### Affine transformations

Arithmetic transformations by a constant. Each is a bijection with a
known inverse.

- `x + k` — inverse: `x - k`
- `x * k` (k ≠ 0) — inverse: `x / k`
- `x * k + c` (k ≠ 0) — inverse: `(x - c) / k`

Homomorphism properties (for `x + k` as representative):
- `==` homomorphic: ✓ — `a == b ⟺ (a+k) == (b+k)`
- `<` homomorphic: ✓ — `a < b ⟺ (a+k) < (b+k)`
- `+` homomorphic: ✗ — `encode(a+b) = a+b+k ≠ encode(a)+encode(b) = a+b+2k`

Note: `x * k` for negative `k` reverses ordering (`<` not homomorphic).

### Product (record)

Packs/unpacks multiple independent values into a fixed-size tuple.

- **Encode:** `Expr::Record` — combine values into a tuple
- **Decode:** `Expr::Project` — extract by index

Bijective because each component is independent and individually recoverable.
Used for embedded structs (fields flattened into columns).

### Coproduct (tagged union)

Encodes/decodes a discriminated union (enum) where the discriminant partitions
the domain into disjoint subsets.

- **Encode:** `Expr::Project` — extract discriminant and per-variant fields
- **Decode:** `Expr::Match` — branch on discriminant, reconstruct variant via
  `Expr::Record`

Bijective if and only if:
- Arms are **exhaustive** (cover all discriminant values)
- Arms are **disjoint** (no overlapping discriminants)
- Each arm's body is **individually a bijection**

This is a coproduct of bijections: if `f_i: A_i → B_i` is a bijection for each
variant `i`, the combined mapping on the tagged union `Σ_i A_i → Σ_i B_i` is
also a bijection.

## Operator Homomorphism

### Operator inventory

Current Toasty binary operators (`BinaryOp`): `==`, `!=`, `<`, `<=`, `>`, `>=`.

Arithmetic operators (`+`, `-`) are not yet in the AST but are needed for
computed fields and interval arithmetic.

For homomorphism analysis, `!=` is the negation of `==`, and `>=`/`<=` are
derivable from `<`/`>`. So the independent set is: **`==`**, **`<`**, **`+`**.

### Per-primitive homomorphism

**Type reinterpretation:**

| Encoding             | `==` | `<`    | `+` |
|----------------------|------|--------|-----|
| Timestamp ↔ String   | ✓    | ✓ (¹)  | ✗   |
| Uuid ↔ String        | ✓    | ✗      | n/a |
| Uuid ↔ Bytes         | ✓    | ✗      | n/a |
| Date ↔ String        | ✓    | ✓ (¹)  | ✗   |
| Time ↔ String        | ✓    | ✓ (¹)  | ✗   |
| DateTime ↔ String    | ✓    | ✓ (¹)  | ✗   |
| Zoned ↔ String       | ✓    | ✗      | ✗   |
| Timestamp ↔ DateTime | ✓    | ✓      | ✓   |
| Timestamp ↔ Zoned    | ✓    | ✓      | ✓   |
| Zoned ↔ DateTime     | ✓    | ✓      | ✓   |
| Decimal ↔ String     | ✓    | ✗      | ✗   |
| BigDecimal ↔ String  | ✓    | ✗      | ✗   |
| Integer widening     | ✓    | ✓      | ✓   |

(¹) Requires canonical fixed-width serialization format. Lexicographic ordering
matches semantic ordering only if Toasty guarantees consistent formatting (no
variable-length subsecond digits, no timezone offset variations, etc.).

All type reinterpretations are injective, so `==` is always preserved. `<` and
`+` depend on whether the target type's native operations align with the source
type's semantics.

**Affine transformations:**

| Encoding       | `==` | `<`          | `+` |
|----------------|------|--------------|-----|
| `x + k`        | ✓    | ✓            | ✗   |
| `x * k` (k>0)  | ✓    | ✓            | ✗   |
| `x * k` (k<0)  | ✓    | ✗ (reversed) | ✗   |
| `x * k + c`    | ✓    | ✓ if k>0     | ✗   |

**Product (record):**

| Operator | Homomorphic? |
|----------|-------------|
| `==`     | ✓ — if each component preserves `==` |
| `<`      | conditional — requires lexicographic comparison and each component preserves `<` |
| `+`      | ✓ — if each component preserves `+` (component-wise) |

**Coproduct (tagged union):**

| Operator | Homomorphic? |
|----------|-------------|
| `==`     | ✓ — if discriminant + each arm preserves `==` |
| `<`      | generally ✗ — cross-variant comparison is usually meaningless |
| `+`      | ✗ — arithmetic across variants undefined |

### Homomorphism under composition

**Sequential** (`g ∘ f`): if both `f` and `g` are homomorphic for an operator,
so is the composition. Proof: `a op b ⟺ f(a) op f(b) ⟺ g(f(a)) op g(f(b))`.

**Parallel/product** (`(f(a), g(b))`): preserves `==` if both `f` and `g` do.
Preserves `<` only if tuple comparison is lexicographic and both preserve `<`.

**Coproduct**: preserves `==` if each arm does. Does not generally preserve `<`.

### Cross-encoding comparisons

When two operands use **different** encodings (e.g., field₁ uses Timestamp→i64,
field₂ uses Timestamp→i64+offset), `can_distribute` does not directly apply.
The comparison `encode₁(a) op encode₂(b)` mixes two encodings and may not
preserve semantics.

Fallback: decode both to model space and compare there.

```
decode₁(col₁) op decode₂(col₂)
```

This always produces correct results but may require SQL CAST or application-side
evaluation.

### Database independence

`can_distribute` does not take a database parameter. Database capabilities
determine **which bijection** is selected (e.g., PostgreSQL has native timestamps
→ identity mapping; SQLite does not → Timestamp↔i64). Once the bijection is
chosen, `can_distribute` is purely a property of that bijection and the operator.

The only edge case is if two databases use the same types but their operators
behave differently (e.g., string collation affecting `<`). This can be handled by
treating such behavioral differences as part of the encoding rather than adding a
database parameter.

## Precision / Domain Restriction

Lossy encodings like `#[column(type = timestamp(2))]` involve two distinct steps:

1. **Domain restriction** (lossy, write-time): the user's full-precision value is
   truncated to the representable domain. This is many-to-one — multiple inputs
   collapse to the same output. It is **not** part of the mapping.

2. **Encoding** (bijective): over the restricted domain (values with ≤2 fractional
   digits), the mapping is a perfect bijection — lossless round-trip.

The mapping framework only governs step 2. Step 1 is a write-time concern:
when the user assigns a value, it gets projected into the representable domain.
Analogous to integer narrowing (`i64 → i32`): the mapping between `i32` values
and the stored column is bijective; the loss happens if you store a value outside
`i32` range.

## Nullability

`Option<T>` with `None → NULL` is a coproduct bijection:

- **Domain partition:** `Option<T> = None | Some(T)` — two disjoint cases.
- **Encoding:** `None → NULL`, `Some(v) → encode(v)` — each arm is individually
  bijective (unit↔NULL is trivially so; `Some` delegates to `T`'s encoding).
- **Decoding:** `NULL → None`, `non-NULL → Some(decode(v))`.

This satisfies the coproduct conditions (exhaustive, disjoint, per-arm bijective).

### NULL breaks standard `==`

SQL uses three-valued logic: `NULL = NULL` evaluates to `NULL` (falsy), not
`TRUE`. This means the standard `==` operator is **not** homomorphic over the
nullable encoding — the model-level `None == None` is `true`, but
`NULL = NULL` is not.

### NULL-safe operators

All Toasty target databases provide a NULL-safe equality operator:

| Database   | Operator                  |
|------------|---------------------------|
| PostgreSQL | `IS NOT DISTINCT FROM`    |
| MySQL      | `<=>`                     |
| SQLite     | `IS`                      |

Using the NULL-safe operator restores `==` homomorphism:
`a == b ⟺ encode(a) IS NOT DISTINCT FROM encode(b)`.

### Operator mapping

This means homomorphism is not just a property of `(encoding, operator)` — it is
a property of the triple `(encoding, model_op, storage_op)`. The lowerer may need
to emit a **different** SQL operator than the one the user wrote:

- Non-nullable field: model `==` → SQL `=`
- Nullable field: model `==` → SQL `IS NOT DISTINCT FROM` (or `<=>`, `IS`)

`can_distribute` should return the storage-level operator to use, not just a
boolean. Signature sketch:

```
can_distribute(encoding, model_op) -> Option<storage_op>
```

`None` means the operator cannot be pushed to the DB. `Some(op)` means it can,
using the specified storage operator.

### Ordering

`NULL` ordering is also database-specific (`NULLS FIRST` vs `NULLS LAST`). The
lowerer must ensure consistent behavior across backends, potentially by emitting
explicit `NULLS FIRST`/`NULLS LAST` clauses.

## Lowering Algorithm

The lowerer transforms a model-level expression tree into a storage-level
expression tree. The input contains field references and model-level literals.
The output contains column references and storage-level values.

### Core: lowering a binary operator

```
lower_binary_op(op, lhs, rhs):
    // 1. Identify field references and look up their encodings
    //    from the schema/mapping.
    lhs_encoding = lookup_encoding(lhs) if lhs is FieldRef, else None
    rhs_encoding = lookup_encoding(rhs) if rhs is FieldRef, else None

    // 2. Determine if the operator can distribute through the encoding.
    //    For single-column primitive encodings:
    if both are FieldRefs with same encoding:
        match can_distribute(encoding, op):
            Some(storage_op):
                // Both fields share the encoding — compare columns directly.
                emit: column_lhs storage_op column_rhs
            None:
                // Decode both to model space.
                emit: decode(column_lhs) op decode(column_rhs)

    if one is FieldRef, other is Literal:
        match can_distribute(field_encoding, op):
            Some(storage_op):
                // Encode the literal, compare in storage space.
                emit: column storage_op encode(literal)
            None:
                // Decode the column to model space.
                emit: decode(column) op literal

    if both are Literals:
        // Const-evaluate in model space.
        emit: literal_lhs op literal_rhs
```

### Encoding the literal

`encode(literal)` applies the field's `field_to_column` bijection to the
model-level value, producing a storage-level value. For a UUID↔text encoding:
`encode(UUID("abc-123"))` → `"abc-123"`.

### Decoding the column

`decode(column_ref)` applies the field's `column_to_field` bijection to the
column reference, wrapping it in the appropriate SQL expression. For UUID↔text:
`decode(uuid_col)` → `CAST(uuid_col AS UUID)`.

If the database lacks the model type (e.g., SQLite has no UUID), decode is not
expressible in SQL. The operation must be evaluated application-side or the query
rejected.

### Multi-column encodings (product / coproduct)

For fields that span multiple columns, `==` expands structurally:

```
lower_binary_op(==, coproduct_field, literal):
    encoded = encode(literal)
    // encoded is a tuple: (disc_val, col1_val, col2_val, ...)

    // Expand into per-column comparisons:
    result = TRUE
    for each (column, encoded_value) in zip(field.columns, encoded):
        col_encoding = encoding_for(column)  // e.g., nullable text
        match can_distribute(col_encoding, ==):
            Some(storage_op):
                result = result AND (column storage_op encoded_value)
            None:
                result = result AND (decode(column) == encoded_value)
    emit: result
```

### ORDER BY

```
lower_order_by(field):
    encoding = lookup_encoding(field)
    match can_distribute(encoding, <):
        Some(_):
            // Ordering is preserved in storage space.
            emit: ORDER BY column
        None:
            // Must decode to model space for correct ordering.
            emit: ORDER BY decode(column)
```

### SELECT returning

Always decode — application needs model-level values:

```
lower_select_returning(field):
    emit: decode(column)  // column_to_field bijection
```

### INSERT / UPDATE

Always encode — database needs storage-level values:

```
lower_insert_value(field, value):
    emit: encode(value)  // field_to_column bijection
```

### Examples

**`WHERE uuid_col == UUID("abc-123")`, UUID stored as text:**

1. LHS is FieldRef → encoding: UUID↔text, column: `uuid_col`
2. RHS is literal: `UUID("abc-123")`
3. `can_distribute(UUID↔text, ==)` → `Some(=)`
4. Encode literal: `"abc-123"`
5. Output: `uuid_col = 'abc-123'`

**`WHERE uuid_col < UUID("abc-123")`, UUID stored as text:**

1. LHS is FieldRef → encoding: UUID↔text, column: `uuid_col`
2. RHS is literal: `UUID("abc-123")`
3. `can_distribute(UUID↔text, <)` → `None`
4. Decode column: `CAST(uuid_col AS UUID)`
5. Output: `CAST(uuid_col AS UUID) < UUID('abc-123')`
6. (If DB lacks UUID type → application-side evaluation or reject)

**`WHERE contact == Contact::Phone { number: "123" }`, coproduct encoding:**

1. LHS is FieldRef → coproduct encoding, columns: `disc`, `phone_number`, `email_address`
2. RHS is literal → encode: `(0, "123", NULL)`
3. Expand per-column:
   - `disc = 0` (`can_distribute(i64, ==)` → `Some(=)`)
   - `phone_number = '123'` (`can_distribute(nullable text, ==)` → `Some(=)`)
   - `email_address IS NULL` (`can_distribute(nullable text, ==)` → `Some(IS)`)
4. Output: `disc = 0 AND phone_number = '123' AND email_address IS NULL`

## Schema Representation

Each field's mapping is stored as a structured `Bijection` tree. This is the
single source of truth — encode/decode expressions are derived from it.

### Bijection enum

```rust
enum Bijection {
    /// No transformation — field type == column type.
    Identity,

    /// Lossless cast between two types with the same information content.
    /// e.g., UUID↔text, Timestamp↔i64, integer widening.
    Cast { from: Type, to: Type },

    /// x*k + c (k ≠ 0). Inverse: (x - c) / k.
    Affine { k: Value, c: Value },

    /// Option<T> → nullable column.
    /// Wraps an inner bijection with None↔NULL.
    Nullable(Box<Bijection>),

    /// Embedded struct → multiple columns.
    /// Each component is an independent bijection on one field↔column pair.
    Product(Vec<Bijection>),

    /// Enum → discriminant column + per-variant columns.
    Coproduct {
        discriminant: Box<Bijection>,
        variants: Vec<CoproductArm>,
    },

    /// Composition: apply `inner` first, then `outer`.
    /// encode = outer.encode(inner.encode(x))
    /// decode = inner.decode(outer.decode(x))
    Compose {
        inner: Box<Bijection>,
        outer: Box<Bijection>,
    },
}

struct CoproductArm {
    discriminant_value: Value,
    body: Bijection, // typically Product for data-carrying variants
}
```

### Methods on Bijection

```rust
impl Bijection {
    /// Encode a model-level value to a storage-level value.
    fn encode(&self, value: Value) -> Value;

    /// Produce a decode expression: given a column reference (or tuple of
    /// column references), return a model-level expression.
    fn decode(&self, column_expr: Expr) -> Expr;

    /// Query whether `model_op` can be pushed through this encoding.
    /// Returns the storage-level operator to use, or None if the
    /// operation must fall back to model space.
    fn can_distribute(&self, model_op: BinaryOp) -> Option<StorageOp>;

    /// Number of columns this bijection spans.
    fn column_count(&self) -> usize;
}
```

`can_distribute` is defined recursively:

- **Identity**: always `Some(model_op)` — no transformation.
- **Cast**: lookup in the per-pair homomorphism table.
- **Affine**: `==` → `Some(=)`. `<` → `Some(<)` if k > 0, `None` if k < 0.
- **Nullable**: delegates to inner, may change op (e.g., `==` → `IS NOT
  DISTINCT FROM`).
- **Product**: `==` → `Some(=)` if all components return `Some`. `<` → only
  if lexicographic and all components support `<`.
- **Coproduct**: `==` → `Some` if discriminant + each arm returns `Some`.
  `<` → generally `None`.
- **Compose**: `Some` only if both inner and outer return `Some`.

### Per-field mapping

```rust
struct FieldMapping {
    bijection: Bijection,
    columns: Vec<ColumnId>, // columns this field maps to (1 for primitive, N for product/coproduct)
}
```

The model-level `mapping::Model` holds a `FieldMapping` per field, plus a
reverse index from columns to fields (for computed field propagation).

## Verification

The framework should be formally verified using Lean 4 + Mathlib. Mathlib already
provides the algebraic vocabulary (bijections, homomorphisms, products,
coproducts). The plan:

1. Define the primitives and composition rules in Lean
2. Prove the general theorems once (composition preserves bijection, coproduct
   conditions, etc.)
3. For each concrete primitive, state and prove its homomorphism properties
4. Lean checks everything mechanically
