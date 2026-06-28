# Document Value Equality Filters

Implements one slice of [document-fields.md](document-fields.md): equality
filters over whole document values.

## Summary

A filter can compare a `#[document]` embed — the whole field, or any
nested embed or collection inside it — against a value, on every target
backend. `User::FIELDS.contact_info().phone().eq(phone)` works the same
way `.area_code().eq("555")` does today. The engine fixes the comparison
semantics once, backend-agnostically: an embed compares field-by-field
against the schema (exact, index-friendly, tolerant of unknown stored
keys), and a collection compares as one structural value. Each backend
then gets its optimal form — native structural equality on PostgreSQL,
MySQL, and DynamoDB; a `json_tree`-based structural comparison on SQLite;
and a transparent in-memory fallback for drivers that advertise no
server-side support.

## Motivation

Document filtering today reaches scalar leaves only. Given:

```rust
#[derive(toasty::Embed)]
struct PhoneNumber {
    area_code: String,
    number: String,
}

#[derive(toasty::Embed)]
struct ContactInfo {
    phone: PhoneNumber,
    emails: Vec<String>,
}

#[derive(toasty::Model)]
struct User {
    #[key] #[auto]
    id: u64,

    #[document]
    contact_info: ContactInfo,
}
```

`.contact_info().phone().area_code().eq("555")` works, but comparing the
embed as a unit does not:

```rust
User::all().filter(
    User::FIELDS.contact_info().phone().eq(PhoneNumber {
        area_code: "555".into(),
        number: "8675309".into(),
    })
);
```

Today this fails on every backend, each in its own way: the SQL edge has
no rendering for a document-typed extraction in a comparison, the
parameter extractor decomposes the embed literal into a row-value tuple,
and the DynamoDB codec panics on an un-typed record. The user has to
spell out one leaf comparison per field by hand — and has no recourse at
all for comparing a `Vec` field, whose length and contents the leaf API
cannot express.

## User-facing API

No new methods. The typed path API users already have gains `.eq()` at
every document position, not just scalar leaves.

### Comparing an embed

Pass a value of the embed's type. The filter matches rows whose stored
embed has every field equal:

```rust
let matches = User::all()
    .filter(User::FIELDS.contact_info().phone().eq(PhoneNumber {
        area_code: "555".into(),
        number: "8675309".into(),
    }))
    .collect(&mut db)
    .await?;
```

This works at any depth — the root document (`contact_info().eq(...)`),
a nested embed (`contact_info().phone().eq(...)`), or an embed inside a
`#[document]` field of a column-expanded embed. `Option` fields follow
the usual rule: a `None` in the comparison value matches a stored `None`.

### Comparing a collection

A `Vec` field inside a document (or a `Vec<embed>` document collection)
compares by value: same length, same elements, same order.

```rust
User::all().filter(
    User::FIELDS.contact_info().emails().eq(vec![
        "a@example.com".to_string(),
        "b@example.com".to_string(),
    ])
);

Order::all().filter(
    Order::FIELDS.items().eq(vec![
        LineItem { sku: "SKU-1".into(), qty: 3 },
    ])
);
```

This complements the element predicates that already exist
(`.contains(...)` asks "is this element present"; `.eq(...)` asks "is
this the exact list").

## Behavior

**Embed equality is schema-shaped.** An embed comparison holds when every
field declared on the embed compares equal, recursively through nested
embeds. Keys present in stored data but absent from the schema (written
by an external client) are ignored — the same closed-schema rule the
decoder applies. A `None` field matches both an absent key and an
explicit null, mirroring `is_none()`.

**Collection equality is structural.** A list comparison holds when both
sides have the same length and equal elements at each index. Inside a
list there is no schema-anchored way to ignore unknown keys server-side,
so an externally-added key inside a stored list element makes the
comparison fail. This is the one semantic seam between the two tiers,
and it is deliberate; see [Edge cases](#edge-cases).

**Where the comparison runs.** On backends that support it (all four
in-tree backends), the whole filter executes server-side in one round
trip. A driver that does not advertise the capability still works: the
engine loads candidate rows and applies the comparison in memory — same
results, more data over the wire. Equality is never rejected with
`unsupported_feature`; rejection is reserved for operators whose
semantics differ across backends (the `.ilike()` rule), and value
equality has one semantics everywhere.

**Result types and errors.** Nothing new: the filter composes with
`filter`, `include`, pagination, and updates-with-filter like any other
predicate. No new error cases for in-tree backends.

## Edge cases

- **Empty collections.** `.eq(vec![])` matches a stored empty list and
  nothing else, on every backend.
- **`Option` leaves.** `Some(x)` requires the stored key present and
  equal; `None` matches absent-or-null. Toasty's writer never produces
  explicit nulls, so the distinction only arises with externally written
  rows.
- **Temporal and decimal leaves.** Both sides encode through the same
  canonical text forms the document codec already uses (fixed sub-second
  precision), so equality on these leaves is exact and consistent with
  the scalar-leaf filters.
- **String comparison inside collections on MySQL.** A record-tier string
  leaf compares under the connection's collation, like a plain column
  (case-insensitive under MySQL's default). A string *inside a list*
  compares as a JSON value — exact, case-sensitive. This follows the
  per-backend pass-through rule: each tier uses that backend's native
  operator for that shape, and the two operators disagree on MySQL.
- **Unknown stored keys.** Tolerated at the record tier (schema-shaped
  comparison), not inside collections (structural comparison). Both
  facts are documented; data written exclusively by Toasty never hits
  the difference.
- **SQLite cost.** Structural collection equality on SQLite walks both
  JSON trees per candidate row. Correctness matches the other backends;
  the cost does not. Rows are filtered server-side, but there is no
  index assist.
- **DynamoDB expression size.** Collection comparisons bind the whole
  value as one expression attribute value, so a large literal does not
  inflate the condition expression itself. DynamoDB's own item and
  expression limits still apply and surface as DynamoDB errors.

## Driver integration

### Capability

One new flag:

```rust
/// Whether the driver can evaluate equality between a document-typed
/// path and a document value server-side (an embed compared as one
/// value, or a collection compared structurally).
pub document_value_eq: bool,
```

All four in-tree backends set it `true` (SQLite via emulation; see
below). A driver that leaves it `false` keeps working: the planner
routes document-value comparisons to the in-memory filter that already
backs post-filtering, and the driver never sees them. Out-of-tree
drivers are therefore unaffected until they opt in.

### What reaches the driver

The engine splits every document comparison into two tiers before any
driver-specific work happens, in the backend-agnostic simplifier:

- **Record tier.** A comparison against an embed decomposes into one
  scalar comparison per schema field, recursively, with `None` fields
  becoming the existing `is_none` form. Drivers see only the
  scalar-leaf shapes they already support — a conjunction of path
  extractions compared to scalar parameters. **No new driver work.**
  This tier is also why decomposition is the default: each leaf can use
  the backend's scalar operators (collation, casts) and any future
  per-path index, and the closed-schema semantics fall out for free.
- **Collection tier.** Decomposition stops at the first `Vec` boundary.
  The residual comparison reaches the driver as an equality between a
  document path and a single document-typed value, named and encoded by
  the engine (the same named form document writes use).

### SQL serialization contract

The collection-tier comparison renders per dialect:

| Dialect | Rendering |
|---|---|
| PostgreSQL | `(col->'a'->'emails') = $1::jsonb` — native structural equality |
| MySQL | `JSON_EXTRACT(col, '$.a.emails') = CAST(? AS JSON)` — native structural equality |
| SQLite | `json_tree` set comparison (below) — emulated structural equality |

SQLite has no structural JSON equality (text comparison is key-order
sensitive), but its `json_tree` table-valued function makes the
emulation exact: two values are equal iff their node sets match.

```sql
NOT EXISTS (
  SELECT fullkey, type, atom FROM json_tree(json_extract(col, '$.emails'))
  EXCEPT
  SELECT fullkey, type, atom FROM json_tree(?)
)
AND NOT EXISTS (
  SELECT fullkey, type, atom FROM json_tree(?)
  EXCEPT
  SELECT fullkey, type, atom FROM json_tree(json_extract(col, '$.emails'))
)
```

`fullkey` carries array indices, so element order matters; object key
order does not. This matches PostgreSQL/MySQL semantics exactly — the
emulation trades cost, not correctness.

### DynamoDB contract

DynamoDB equality is defined for every attribute type, so both tiers are
native: the record tier is the nested-path comparisons the driver
already compiles (`#col.#phone.#area_code = :v0 AND ...`), and the
collection tier is one comparison against a Map/List attribute value
(`#col.#emails = :v` with `:v` an `L`).

### No new operations

No new `Operation` variants. SQL drivers see new expression shapes only
in the statements they already serialize; DynamoDB sees them in the
filter expressions it already compiles.

## Implementation plan

Ordered so each milestone ships working, tested behavior on all four
backends:

1. **Record tier.** The simplifier decomposition plus the path-API
   surface check. Ships embed equality (root and nested) everywhere,
   riding the scalar-leaf machinery that already exists — zero driver
   changes. Integration tests: nested eq, `Option` field as `Some` and
   `None`, near-miss negative.
2. **Collection tier, native backends.** The `document_value_eq`
   capability, planner routing (capability off → in-memory filter),
   engine-side naming of comparison literals, PostgreSQL/MySQL
   serialization, DynamoDB Map/List equality. SQLite ships this
   milestone with the capability off — correct via the in-memory path.
3. **SQLite structural rendering.** The `json_tree` form; flip SQLite's
   capability. A cross-backend invariant test asserts the collection-
   tier comparison agrees with an equivalent decomposed comparison on
   every backend (the `struct_embed_filter_matches_column_case_sensitivity`
   pattern).
4. **Follow-ons.** `ne()` as the negation of the conjunction; two-sided
   comparisons (`a.phone == b.phone`) — the decomposition is
   shape-directed rather than value-directed, so it already applies;
   the work is the query-surface and parameter plumbing.

## Alternatives considered

**Whole-value equality at every tier (no decomposition).** Compare the
embed as one JSON/Map value everywhere. Rejected: it silently changes
semantics from the decoder's closed-schema rule (an unknown stored key
would break equality), it can never use a per-leaf index, record-tier
string leaves would stop following column collation on MySQL, and on
SQLite it would force the expensive `json_tree` form onto the common
case.

**Flattening collection comparisons (length guard plus per-index
leaves).** A constant list fixes its own length, so
`emails == ["a","b"]` *could* decompose into
`len = 2 AND emails[0] = 'a' AND emails[1] = 'b'`. Rejected: the
conjunction grows with the literal (a hard failure against DynamoDB's
4 KB condition-expression cap, not just verbosity), it duplicates
semantics the whole-value form must implement anyway for non-constant
comparisons, and native structural equality already exists on three of
four backends.

**SQLite text equality for the collection tier.** `json_extract(...) = json(?)`
is exact for Toasty-written rows (one canonical encoder on both sides)
but key-order sensitive for externally written ones. Rejected as the
shipping form because it would make SQLite the one backend with
divergent semantics; kept in mind as a fast path if the `json_tree`
form proves too slow, at which point the divergence would need to be a
documented capability, not a silent one.

**Rejecting unsupported drivers instead of in-memory fallback.** The
`.ilike()` precedent rejects, but that rule exists because backends
*disagree* on ilike's semantics. Value equality has one semantics, and
the in-memory evaluator implements it exactly, so transparent fallback
is the established pattern (it already backs post-filters on key-fetch
paths).

## Open questions

- **Capability granularity.** One `document_value_eq` flag, or split
  per shape (embed vs. list vs. future map)? Start with one; split when
  a real driver needs the distinction. Deferrable.
- **`json_tree` rendering under negation and disjunction.** The
  EXCEPT-pair is a pair of scalar subqueries; nesting it inside `OR` /
  `NOT` duplicates it. If that proves unacceptable, milestone 3 narrows
  to top-level conjuncts and other positions stay on the in-memory
  path. Blocking milestone 3 implementation, not acceptance.
- **`ne()` null semantics.** Whether `ne` over an embed with `Option`
  fields means strict negation of `eq` (absent counts as "not equal")
  on every backend, given SQL's three-valued logic. Blocking the
  follow-on, not this design.

## Out of scope

- **Sub-document containment (`partial!`).** "Does the document contain
  this shape" is a different operator with its own design in
  [document-fields.md](document-fields.md); this design only covers
  exact equality.
- **Map and set field types.** Not field types yet. The two-tier rule
  extends to them (maps join the collection tier), but their design
  lands with the field types.
- **Ordering comparisons.** `lt` / `gt` on document values have no
  meaningful cross-backend semantics and are not planned.
