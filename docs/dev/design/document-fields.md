# Document and Collection Fields

Subsumes the [JSON field queries roadmap entry](../roadmap.md#query-engine).

## Summary

Toasty stores `#[derive(Embed)]` structs as single document columns under
the `#[document]` attribute, stores `Vec` collections of embeds as
document arrays, and filters on scalar leaves inside those documents on
all four target backends. This design covers what remains: equality
filters over whole document values, map and set field types, sub-document
containment, per-element predicates, in-place document mutations, the
`#[document]` shapes still rejected today, and indexing for collection
and document fields — with an API an eventual MongoDB driver can adopt
without new user-facing concepts.

## Current state

What ships today, as context; the rest of the document designs only what
is missing.

- Embeds column-expand by default (one column per leaf field; one
  top-level attribute on DynamoDB). `#[document]` collapses an embed
  struct into one document column — `jsonb` on PostgreSQL, `JSON` on
  MySQL, JSON text on SQLite, a Map `M` attribute on DynamoDB — at the
  model root or on a field nested inside a column-expanded embed. A
  `Vec` of embeds stores as a document array (`L` of `M` on DynamoDB)
  with or without the attribute. `Capability::document_collections`
  gates document storage; all four in-tree backends set it.
- Filters reach scalar leaves inside a document at any depth — equality,
  ordering, `is_none()` — and temporal and decimal leaves compare
  exactly through the document codec's canonical text forms. Document
  fields compose with create, whole-value update, and `include`.
- `Vec<scalar>` model fields store as `text[]` on PostgreSQL, JSON on
  MySQL and SQLite, and a List on DynamoDB. They expose the `contains` /
  `is_superset` / `intersects` / `len` / `is_empty` predicates and the
  `stmt::push` / `stmt::extend` / `stmt::clear` mutations on every
  backend (`stmt::push` also appends to document collections);
  `stmt::pop` / `stmt::remove` / `stmt::remove_at` ship for PostgreSQL
  native arrays only; every other combination — other backends, and
  document collections everywhere — is rejected at lowering.
- Documents cross the driver boundary as named `Value::Object` values in
  `db::Type::Document { binary }` columns. Drivers encode and decode
  documents shape-directed — interior leaves take their wire forms — and
  never consult the application schema.
- The gaps hold explicit rejections rather than misbehavior: comparing a
  document path to a whole value fails with `unsupported_feature`;
  `#[document(text)]` and `#[index]` / `#[unique]` / `#[column]` on a
  `#[document]` field are compile errors; `#[document]` on anything
  other than an embed struct or a `Vec` of them fails the `Document`
  trait bound; enum embeds, `Zoned`, and `Vec<u8>` leaves inside a
  document are schema-build errors.

## Motivation

Four gaps remain, and all four come up constantly:

- **No whole-value comparison.** `.preferences().theme().eq("dark")`
  works, but comparing an embed or a `Vec` as a unit is rejected. The
  user has to spell out one leaf comparison per field by hand — and has
  no recourse at all for a `Vec` field, whose length and contents the
  leaf API cannot express.
- **No map or set field types.** `HashMap<String, T>`, `HashSet<T>`, and
  their `BTree` forms are not accepted as model fields. Open-ended
  attributes (user preferences, feature flags, audit context) and
  tag-like collections with set semantics have no home.
- **Every document mutation is whole-value replacement.** Changing one
  field inside a `#[document]` embed means writing the entire document
  back. PostgreSQL's `jsonb_set`, DynamoDB's `SET path = :v`, and
  MySQL / SQLite's `JSON_SET` all support in-place patching that Toasty
  cannot reach.
- **No containment, element predicates, or indexes.** "Does this
  document contain this shape," "does any element match this predicate,"
  and "make filtering on this document leaf fast" are all inexpressible.

## User-facing API

### Comparing a document value

No new methods. The typed path API gains `.eq()` at every document
position, not just scalar leaves.

#### Comparing an embed

Pass a value of the embed's type. The filter matches rows whose stored
embed has every field equal:

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

let matches = User::all()
    .filter(User::fields().contact_info().phone().eq(PhoneNumber {
        area_code: "555".into(),
        number: "8675309".into(),
    }))
    .collect(&mut db)
    .await?;
```

This works at any depth — the root document
(`contact_info().eq(...)`), a nested embed
(`contact_info().phone().eq(...)`), or an embed inside a `#[document]`
field of a column-expanded embed. `Option` fields follow the usual
rule: a `None` in the comparison value matches a stored `None`.

#### Comparing a collection

A `Vec` field inside a document (or a `Vec` document collection)
compares by value: same length, same elements, same order.

```rust
User::all().filter(
    User::fields().contact_info().emails().eq(vec![
        "a@example.com".to_string(),
        "b@example.com".to_string(),
    ])
);

Order::all().filter(
    Order::fields().items().eq(vec![
        LineItem { sku: "SKU-1".into(), qty: 3 },
    ])
);
```

This complements the element predicates: `.contains(...)` asks "is this
element present"; `.eq(...)` asks "is this the exact list."

### Sub-document containment

Containment asks "does the document contain this shape?" Build a
partial value of the embed's type with the `partial!` macro:

```rust
User::all().filter(
    User::fields().preferences().contains(toasty::partial!({
        theme: "dark",
    }))
);
```

The signature is `.contains(impl Into<Expr<Partial<T>>>) -> Expr<bool>`,
where `T` is the embed type. `Partial<T>` is a thin wrapper carrying the
type parameter for type-checking; the `partial!` macro produces one from
struct-literal syntax. Field names not in `{ ... }` are absent from the
predicate, so `partial!({ theme: "dark" })` matches any row whose
`preferences.theme` is `"dark"`, regardless of other fields. Nested
partial values work the same way:

```rust
User::all().filter(
    User::fields().preferences().contains(toasty::partial!({
        notifications: { email: true },
    }))
);
```

Internally, the literal lowers to a nested `stmt::Record` carrying only
the named field-value pairs. `partial!` validates field names against
the schema at runtime; a compile-time check is a DX nicety that can
follow.

Containment is a document-storage feature; the query engine rejects
`.contains(...)` on a column-expanded embed with an error pointing at
`#[document]`.

### Map and set fields

`HashMap<String, T>`, `BTreeMap<String, T>`, `HashSet<T>`, and
`BTreeSet<T>` become model and embed fields with no attribute, storage
backend-chosen per the table in [Behavior](#behavior):

```rust
#[derive(toasty::Model)]
struct User {
    #[key] #[auto]
    id: u64,

    permissions: HashSet<String>,        // text[] on PG, typed Set on DynamoDB
    metadata: HashMap<String, String>,   // jsonb on PG, JSON elsewhere
}
```

`User::fields().metadata()` is a typed path of key type `String` and
value type `String`:

- `.contains_key(impl Into<Expr<K>>) -> Expr<bool>` — the map has this
  key. (`HashMap::contains_key`.)
- `.keys()` — a set view exposing the same set methods as collection
  paths (`.contains`, `.is_superset`, `.intersects`).
- `.values()` — an iterable view supporting `.any(|v| …)` and
  `.all(|v| …)` (see [Element predicates](#element-predicates)).
- `.len()`, `.is_empty()`.

The collection predicates and mutations that `Vec<scalar>` already
exposes carry over to `HashSet` unchanged; the new piece on the set side
is the marker type that types their right-hand sides.

#### The `Set<T>` marker

`Set<T>` marks set-shaped expressions, analogous to the existing
`List<T>` marker for ordered collections. The set predicates
(`.is_superset`, `.intersects`) and set mutations take `Set<T>` on the
right-hand side; common Rust types implement the relevant `Into` impl so
values pass through directly. `Vec<T>`, `&[T]`, `[T; N]`, `HashSet<T>`,
and `BTreeSet<T>` all satisfy `Into<Set<T>>` and `Into<List<T>>` where
applicable.

### Remaining `#[document]` shapes

`#[document]` accepts an embed struct or a `Vec` of them today. This
design extends it to:

- **`Option<T>`** of any accepted shape — an optional document column
  (SQL `NULL` when `None`).
- **`Vec<scalar>`, `HashSet<T>`, and map types** — forces document
  encoding over the backend's native representation. Useful when the
  same model is shared across backends and needs uniform query
  semantics, or on DynamoDB to force a List over a typed Set.
- **Bare scalars** — one document column that accepts multiple value
  types.
- **Enum embeds** — encoded with an internal discriminator; see
  [Behavior](#behavior).
- **Recursive embeds.** An embed that contains itself, directly or
  through another embed, cannot be column-expanded — the schema would be
  infinite. `#[document]` on the recursive field collapses the cycle
  into one document slot:

  ```rust
  #[derive(toasty::Embed)]
  struct Node {
      value: i64,

      #[document]
      parent: Option<Box<Node>>,
  }
  ```

  The user must write `#[document]` explicitly — Toasty does not infer
  it. Implicit auto-switching would silently change the storage shape;
  the explicit attribute keeps the user in control of where the cycle is
  broken, which matters for mutually recursive types with multiple
  possible cycle points.
- **`#[document(text)]`** — selects PostgreSQL's text `json` over
  `jsonb`; other backends ignore the modifier. Reach for it only when
  exact-byte preservation matters — audit trails, third-party JSON
  re-emitted verbatim. The attribute parses today and is rejected until
  the text-encoding path is wired up.

### Element predicates

`any` and `all` already evaluate predicates over the children of a
`has_many` relation. The same surface extends to the elements of a
document-stored or embedded array:

```rust
Order::all().filter(Order::fields().line_items().any(|i|
    i.product_id().eq(123).and(i.quantity().gt(0))
));

Order::all().filter(Order::fields().line_items().all(|i|
    i.in_stock().eq(true)
));
```

And to the keys and values of a map:

```rust
User::all().filter(
    User::fields().metadata().keys().any(|k| k.starts_with("internal_"))
);
User::all().filter(
    User::fields().metadata().values().any(|v| v.eq("ok"))
);
```

`any` runs server-side on every SQL backend (`EXISTS` over
`jsonb_array_elements` or `unnest` on PostgreSQL, `json_each` on SQLite,
`JSON_TABLE` on MySQL); `all` compiles to `NOT any(NOT pred)`. DynamoDB
has no per-element predicate in its expression language and falls back
to client-side filtering.

### Distinguishing absent from null

`is_none()` matches both an absent key and an explicit JSON null.
Toasty's writer never produces explicit nulls, so the distinction only
arises with externally written rows. Two new predicates split the cases:
`is_absent()` matches only absent keys; `is_null()` matches only
explicit nulls.

### Updating document-stored fields

Whole-value replacement already works. The remaining surface mutates a
document in place.

#### Nested patch via `stmt::patch`

`stmt::patch(path, value)` updates one field inside a value and leaves
the rest unchanged. It already applies to column-expanded embeds, where
it translates to a column-level update. The remaining work is the
document-stored case, where it compiles to an in-place document
mutation:

```rust
user.update()
    .preferences(stmt::patch(
        UserPreferences::fields().theme(),
        "light",
    ))
    .exec(&mut db)
    .await?;
```

On PostgreSQL `jsonb` this lowers to `jsonb_set`; on DynamoDB to
`SET path = :v`; on MySQL and SQLite to `JSON_SET` / `json_set`.

#### Numeric increment

`stmt::increment` already increments scalar and column-expanded fields.
The remaining work is incrementing a numeric leaf inside a document:
atomic on backends with a server-side form (PostgreSQL `jsonb_set` with
a computed expression, DynamoDB `ADD`, MongoDB `$inc`), read-modify-write
otherwise.

#### Set mutations

`HashSet<T>` / `Set<T>` paths expose in-place mutations that match
`std::HashSet`:

- `stmt::insert(impl Into<Expr<T>>)` — add one element.
- `stmt::extend(impl Into<Set<T>>)` — add many.
- `stmt::remove(impl Into<Expr<T>>)` — remove the matching element.
- `stmt::clear()` — remove all elements.

```rust
user.update().permissions(stmt::insert("admin")).exec(&mut db).await?;
user.update().permissions(stmt::remove("guest")).exec(&mut db).await?;
```

Each function is typed to its collection: `stmt::push` on a `HashSet`
path is a compile error (use `stmt::insert`), and `stmt::insert` on a
`Vec` path is a compile error (use `stmt::push`). Today the `insert` /
`remove` / `extend` / `clear` functions are typed to `List<T>`;
introducing the `Set<T>` marker is what makes the split possible. The
lowering uses the backend's native operation where one exists
(`array_append`, `||`, DynamoDB typed-Set `ADD` / `DELETE`) and falls
back to read-modify-write where it does not.

#### Completing the `Vec` mutations

`stmt::pop`, `stmt::remove`, and `stmt::remove_at` work atomically on
PostgreSQL native arrays only; other backends and document collections
reject them at lowering. Completing this design brings them to the
rest: PostgreSQL
document collections get `jsonb` forms, MySQL and SQLite compile them to
a whole-document rewrite (atomic per row; cost scales with document
size), DynamoDB to native update expressions (`REMOVE path[i]`,
typed-Set `DELETE`), with a read-modify-write fallback where no native
operator exists (value removal on a DynamoDB List).

#### Removing a key

`stmt::unset(path)` removes a key from a document (`-` on PostgreSQL
`jsonb`, `JSON_REMOVE` / `json_remove` on MySQL / SQLite,
`REMOVE path.key` on DynamoDB). It is not yet exposed in the public
`stmt` surface.

### Indexes

The existing `#[index]` attribute extends to collection and document
fields. The user-facing rule is unchanged: `#[index]` on a field means
"filtering on this field is fast." Toasty picks the index kind per
backend from the field's type and storage. The attribute is
storage-independent: `#[index]` on a scalar leaf of an embed means the
same thing whether the parent is column-expanded or document-stored —
only the lowering changes.

| Where `#[index]` sits | Per-backend lowering |
|---|---|
| Scalar inside a `#[document]` embed | B-tree expression index on the extracted path (PG/MySQL/SQLite); path index (Mongo); schema-build error on DynamoDB unless denormalized |
| `Vec<T>` or `HashSet<T>` field | GIN with `array_ops` on `text[]` / `jsonb_ops` on `jsonb` (PG); multi-valued (MySQL 8.0+); multikey (Mongo); schema-build error (SQLite, DynamoDB) |
| `HashMap<String, T>` field | GIN with `jsonb_ops` (PG); wildcard (Mongo); schema-build error (MySQL, SQLite, DynamoDB) |
| Whole `#[document]` embed | GIN with `jsonb_ops` for containment (PG); wildcard (Mongo); schema-build error elsewhere |

Three notes:

- **Method generation.** `#[index]` on a leaf inside an embed type does
  not generate a `filter_by_*` method on the parent model — the embed
  macro cannot see the parent's identifier. The index DDL emits
  regardless; the user filters via the path API, which the engine routes
  through the index.
- **Schema-build errors.** When a backend has no viable index type for a
  field shape, Toasty rejects the schema with a message naming the
  field, the backend, and the constraint. Silent degradation is worse
  than an error here because the user wrote `#[index]` expecting the
  index to exist.
- **Backend-specific tuning** (GIN opclass selection, partial-index
  conditions, wildcard projection) is out of scope; a future modifier
  syntax like `#[index(opclass = "jsonb_path_ops")]` covers the long
  tail.

`#[unique]` on a `Vec<T>` is a schema-build error (use `HashSet<T>` if
uniqueness is the intent; PG-side enforcement is an open question).

## Behavior

**Document value equality is two-tiered.** An embed comparison is
schema-shaped: it holds when every field declared on the embed compares
equal, recursively through nested embeds. Keys present in stored data
but absent from the schema (written by an external client) are ignored —
the same closed-schema rule the decoder applies. A `None` field matches
both an absent key and an explicit null, mirroring `is_none()`. A
collection comparison is structural: it holds when both sides have the
same length and equal elements at each index. Inside a list there is no
schema-anchored way to ignore unknown keys server-side, so an
externally-added key inside a stored list element makes the comparison
fail. This is the one semantic seam between the two tiers, and it is
deliberate; see [Edge cases](#edge-cases).

**Where equality runs.** On backends that support it (all four in-tree
backends), the whole filter executes server-side in one round trip. A
driver that does not advertise the capability still works: the engine
loads candidate rows and applies the comparison in memory — same
results, more data over the wire. Equality is never rejected with
`unsupported_feature`; rejection is reserved for operators whose
semantics differ across backends (the `.ilike()` rule), and value
equality has one semantics everywhere. The filter composes with
`filter`, `include`, pagination, and updates-with-filter like any other
predicate; no new error cases for in-tree backends.

**Storage selection for the new field types.** Chosen per (backend,
field type) at schema build time, observable through the column type but
not through the query API:

| Field type | PostgreSQL | MySQL | SQLite | MongoDB | DynamoDB |
|---|---|---|---|---|---|
| `HashSet<scalar>` | `T[]` | `JSON` | JSON1 | BSON array | typed Set `SS`/`NS`/`BS` |
| `HashMap<String, T>` / `BTreeMap` | `jsonb` | `JSON` | JSON1 | BSON sub-document | Map `M` |

DynamoDB typed Sets support string, numeric, and binary element types
only. `HashSet<bool>`, `HashSet<struct>`, and other element types fall
back to List `L` storage on DynamoDB and lose the atomic `ADD` /
`DELETE` path. The fallback is silent; users who need atomicity should
pick a scalar element type.

**Encoding.** Map and set values encode each element or entry with the
same representation Toasty uses for a standalone column of that type,
packed into the backend's document container — the rule document
collections already follow.

**Enum discriminators.** Enum embeds inside documents use internal
tagging with the key `type` — the canonical serde convention
(`#[serde(tag = "type")]`). Internal tagging beats serde's default
external tagging because external adds a nesting level per variant, and
DynamoDB caps Map/List nesting at 32 levels — a budget that nested
embeds with enums at multiple levels can exhaust. A variant field whose
name resolves to `type` collides with the discriminator and is rejected
at schema build time.

**Null vs missing key.** `Option<T>` writes nothing for `None` and a
document value for `Some`. On read, both an absent key and an explicit
JSON `null` deserialize to `None`. `is_none()` matches both;
`is_absent()` and `is_null()` split the cases.

**Patch semantics.** `stmt::patch(path, value)` writes `value` at
`path`, creating intermediate objects as needed. A patch that walks
through a field whose current type is incompatible (patching
`notifications.email` when `notifications` is currently a JSON array)
returns a runtime error on the affected row. Toasty does not validate
the existing document before patching, so a patch onto a row written by
an older or external writer can produce surprising shapes.

**Collection writes.** `stmt::insert` adds an element, creating the set
if absent. `stmt::remove(value)` removes every element equal to the
value (Vec) or the matching element (Set); absent is a no-op.
`stmt::remove_at(idx)` removes a Vec element by index; out-of-bounds is
a no-op, since per-row failure semantics on a bulk update are rarely
useful. `stmt::pop` removes the last Vec element; empty is a no-op.

**Concurrent updates.** A single Toasty operation against one row is
atomic on every backend — the row write lock serializes concurrent
operations, and on SQL backends `READ COMMITTED` makes the second
writer's UPDATE re-read the column at execution time. Two writers
patching disjoint paths on the same row both land. The exception is the
read-modify-write fallback marked in the
[Capability matrix](#capability-matrix): RMW splits the operation into a
SELECT and an UPDATE, so a concurrent writer can interleave between them
and the second commit overwrites a value computed from a stale read. RMW
operations need an explicit transaction with row locking (e.g.
`SELECT … FOR UPDATE`) when concurrent correctness matters.

## Edge cases

- **Empty collections.** `.eq(vec![])` matches a stored empty list and
  nothing else, on every backend.
- **`Option` leaves under equality.** `Some(x)` requires the stored key
  present and equal; `None` matches absent-or-null. Toasty's writer
  never produces explicit nulls, so the distinction only arises with
  externally written rows.
- **Temporal and decimal leaves.** Both sides of a comparison encode
  through the same canonical text forms the document codec already uses
  (fixed sub-second precision), so equality on these leaves is exact and
  consistent with the scalar-leaf filters that ship today.
- **String comparison inside collections on MySQL.** A record-tier
  string leaf compares under the connection's collation, like a plain
  column (case-insensitive under MySQL's default). A string *inside a
  list* compares as a JSON value — exact, case-sensitive. This follows
  the per-backend pass-through rule: each tier uses that backend's
  native operator for that shape, and the two operators disagree on
  MySQL.
- **Unknown stored keys.** Tolerated at the record tier (schema-shaped
  comparison), not inside collections (structural comparison). Both
  facts are documented; data written exclusively by Toasty never hits
  the difference.
- **SQLite collection-equality cost.** Structural collection equality on
  SQLite walks both JSON trees per candidate row. Correctness matches
  the other backends; the cost does not. Rows are filtered server-side,
  but there is no index assist.
- **DynamoDB expression size.** Collection comparisons bind the whole
  value as one expression attribute value, so a large literal does not
  inflate the condition expression itself. DynamoDB's own item and
  expression limits still apply and surface as DynamoDB errors.
- **Mixed-type values at a path.** Filtering a map value with `.eq(5)`
  matches the JSON number 5; it does not coerce the JSON string `"5"`.
  Strict typing is the default.
- **Empty PG arrays.** `array_length(col, 1)` returns `NULL` for an
  empty array; `.len()` on `text[]`-stored sets lowers to
  `cardinality(col)`, which returns 0.
- **Document size limits.** PostgreSQL TOAST caps values near 1 GB;
  MongoDB caps documents at 16 MB; DynamoDB caps items at 400 KB. Toasty
  does not enforce a smaller limit; oversized writes surface a driver
  error.

## Driver integration

### Document value equality

#### Capability

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

#### What reaches the driver

The engine splits every document comparison into two tiers before any
driver-specific work happens, in the backend-agnostic simplifier:

- **Record tier.** A comparison against an embed decomposes into one
  scalar comparison per schema field, recursively, with `None` fields
  becoming the existing `is_none` form. Drivers see only the scalar-leaf
  shapes they already support — a conjunction of path extractions
  compared to scalar parameters. **No new driver work.** This tier is
  also why decomposition is the default: each leaf can use the backend's
  scalar operators (collation, casts) and any future per-path index, and
  the closed-schema semantics fall out for free.
- **Collection tier.** Decomposition stops at the first `Vec` boundary.
  The residual comparison reaches the driver as an equality between a
  document path and a single document-typed value, named and encoded by
  the engine (the same named form document writes use).

#### SQL serialization contract

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
order does not. This matches PostgreSQL / MySQL semantics exactly — the
emulation trades cost, not correctness.

#### DynamoDB contract

DynamoDB equality is defined for every attribute type, so both tiers are
native: the record tier is the nested-path comparisons the driver
already compiles (`#col.#phone.#area_code = :v0 AND ...`), and the
collection tier is one comparison against a Map / List attribute value
(`#col.#emails = :v` with `:v` an `L`).

No new `Operation` variants: SQL drivers see new expression shapes only
in statements they already serialize; DynamoDB sees them in the filter
expressions it already compiles.

### New operations for containment, predicates, and mutations

The rest of the surface adds statement nodes for sub-document
containment, key existence, element predicates, and the document
mutations (`patch` into a document path, `unset`, set operations).
Operations that exist in both native-array and document-storage forms
share one variant carrying a storage-kind hint:

```rust
struct ExprContains {
    lhs: Box<Expr>,
    rhs: Box<Expr>,
    kind: CollectionKind,   // NativeArray or Document
}
```

Today each SQL flavor serializes a collection operator for the one
storage its `Vec<scalar>` columns use, so no hint is needed. Once
`#[document]` can force JSON storage on PostgreSQL, the same flavor must
render both forms (`'x' = ANY(col)` vs `col @> '["x"]'::jsonb`), and the
lowering sets `kind` from the column's storage. The eval interpreter
ignores `kind` — by the time data reaches eval it is decoded Rust
values.

Each operation is gated behind a capability flag. The planner reads
capabilities to decide whether to push the operator to the driver or
fall back to an in-memory implementation. Drivers that implement none of
the new capabilities still work — every predicate compiles to
load-and-filter, every update compiles to read-modify-write.

Per-dialect contract for the operators this design adds:

| Operation | PG `text[]` | PG `jsonb` | SQLite (JSON1) | MySQL | DynamoDB |
|---|---|---|---|---|---|
| Containment (`partial!`) | n/a | `@>` | `json_each` + filter | `JSON_CONTAINS` | AND of path equalities |
| `contains_key` | n/a | `?` | `json_extract IS NOT NULL` | `JSON_CONTAINS_PATH` | `attribute_exists(path)` |
| `any` element predicate | `EXISTS unnest` | `EXISTS jsonb_array_elements` | `EXISTS json_each` | `JSON_TABLE` | client-side filter |
| Patch one path | column update | `jsonb_set` | `json_set` | `JSON_SET` | `SET path = :v` |
| Increment (nested) | column update | `jsonb_set` with cast | `json_set` arith | `JSON_SET` arith | `ADD path :n` |
| `insert` (set) | conditional `array_append` | `\|\|` | rewrite | rewrite | `ADD` (typed Set) |
| `pop` | array slicing | `jsonb_set` w/ length-1 | rewrite | rewrite | `REMOVE path[size-1]` |
| `remove` (by value) | `array_remove` | RMW | rewrite | rewrite | `DELETE path :s` (typed Set) / RMW (List) |
| `remove_at` | array slicing | `jsonb_path` minus | rewrite | `JSON_REMOVE` | `REMOVE path[i]` |
| `unset` (key) | n/a | `-` | `json_remove` | `JSON_REMOVE` | `REMOVE path.key` |

("rewrite" = whole-document rewrite in one statement, atomic per row;
"RMW" = read-modify-write, two round trips, not atomic across writers.)

### MySQL driver

Map and set fields store as `JSON`, the column type document storage
already uses. The work is the document-form lowering of the operators
above and the rewrite fallbacks for `pop` / `remove` / `remove_at`.

### DynamoDB driver

- Storage routing for the new field types: typed Set (`SS`/`NS`/`BS`)
  for eligible `HashSet` element types, List otherwise, Map `M` for
  maps.
- Compilation of the new condition-expression forms (AND-of-`contains`
  for `is_superset` over sets, `attribute_exists` for `contains_key`).
- Compilation of `stmt::Assign` nodes to update expressions (`SET`,
  `ADD`, `DELETE`, `REMOVE`).
- Known gaps, all documented: no sub-document containment operator
  (containment lowers to an AND of path equalities, which does not match
  "any matching shape anywhere"); no per-element predicate (`any` /
  `all` fall back to client-side filtering); no value removal on Lists
  (RMW); GSI / LSI keys must be top-level scalars, so document paths are
  not indexable without denormalization; filter expressions do not
  reduce consumed read capacity.

### MongoDB driver (future)

Toasty has no MongoDB driver yet; the API here is designed so adding one
requires no new user-facing concepts. The driver compiles statement
nodes directly to query and update documents; SQL serialization does not
apply. Items a future driver must work through:

- **BSON-only types in `#[document]` embeds.** `ObjectId`, `Date`,
  `Decimal128` have no JSON representation and need native BSON
  encoding; until then, embeds containing them are rejected at schema
  build.
- **Positional array operators.** `stmt::patch` over an array path needs
  `$[<id>]` / `arrayFilters` to stay atomic.
- **Map keys containing `.`.** Mongo path notation uses `.` as a
  separator; arbitrary string keys need escaping or rejection.
- **Wildcard / multikey index DDL** for the index table above.

### Out-of-tree drivers

Existing drivers compile unchanged. Every new operation is gated behind
a capability flag; absent flags fall back to load-and-filter or
read-modify-write through the existing `QuerySql` and `Insert`
operations.

## Capability matrix

The target end-state per backend for the operators this design adds. The
user-facing API is the same in every column; the matrix captures cost
and atomicity differences only.

| Capability | PostgreSQL | MySQL | SQLite | MongoDB | DynamoDB |
|---|---|---|---|---|---|
| Whole-value equality | native | native | emulated (`json_tree`) | native | native |
| Sub-document containment | native (`@>`) | native (`JSON_CONTAINS`) | compound | native | compound |
| Key existence | native | native | native | native | native |
| `any` / `all` element predicate | native | native | native | native | client-side |
| `stmt::patch` (nested) | native | native | native (rewrite) | native (atomic) | native (atomic) |
| `stmt::increment` (nested) | native | native | native (rewrite) | native (atomic) | native (atomic) |
| `stmt::insert` / `extend` (set) | native | native | native (rewrite) | native (atomic) | native (atomic) |
| `stmt::pop` | native (`text[]` ships; `jsonb` remains) | native (rewrite) | native (rewrite) | native (atomic) | native (atomic) |
| `stmt::remove` (by value) | native (`text[]` ships) / RMW (`jsonb`) | RMW | RMW | native (atomic) | native on typed Set / RMW on List |
| `stmt::remove_at` | native (`text[]` ships; `jsonb` remains) | native (rewrite) | native (rewrite) | native (atomic) | native (atomic) |
| `stmt::unset` | native | native | native (rewrite) | native (atomic) | native (atomic) |
| GIN / wildcard index | ✓ | — | — | ✓ | — |
| Path expression index | ✓ (B-tree) | partial | — | ✓ | denormalize to top-level attr |
| Disjoint-path concurrent writes | last-write-wins | last-write-wins | last-write-wins | independent | independent |

Legend:

- **native** — direct server-side operator, one round trip.
- **compound** — composed from native primitives, server-side, one round
  trip but more expressions evaluated.
- **emulated** — server-side, exact semantics, no native operator (see
  the SQLite `json_tree` form).
- **rewrite** — rewrites the whole document column in one statement.
  Atomic per row; cost scales with document size.
- **RMW** — read-modify-write, two round trips, not atomic across
  concurrent writers without a transaction.
- **client-side** — Toasty fetches and filters in process.

## Implementation plan

Ordered so each milestone ships working, tested behavior on all four
backends:

1. **Equality, record tier.** The simplifier decomposition plus the
   path-API surface check. Ships embed equality (root and nested)
   everywhere, riding the scalar-leaf machinery that already exists —
   zero driver changes. Integration tests: nested eq, `Option` field as
   `Some` and `None`, near-miss negative.
2. **Equality, collection tier on native backends.** The
   `document_value_eq` capability, planner routing (capability off →
   in-memory filter), engine-side naming of comparison literals,
   PostgreSQL / MySQL serialization, DynamoDB Map / List equality.
   SQLite ships this milestone with the capability off — correct via the
   in-memory path.
3. **SQLite structural rendering.** The `json_tree` form; flip SQLite's
   capability. A cross-backend invariant test asserts the collection-
   tier comparison agrees with an equivalent decomposed comparison on
   every backend (the `struct_embed_filter_matches_column_case_sensitivity`
   pattern).
4. **Map and set field types.** Storage selection, encoding, the
   `Set<T>` marker, and the basic predicates (`contains_key`, set views).
5. **Containment and element predicates.** `partial!` / `Partial<T>`,
   `any` / `all`, `is_absent` / `is_null`.
6. **Mutations.** `stmt::patch` and `stmt::increment` into documents,
   set mutations, `pop` / `remove` / `remove_at` beyond PostgreSQL
   `text[]` (lifting the lowering's rejection on document collections
   as each form lands), `stmt::unset`.
7. **Indexing.** The `#[index]` lowering table and its schema-build
   errors.

Follow-ons after milestone 3: `ne()` as the negation of the equality
conjunction, and two-sided comparisons (`a.phone == b.phone`) — the
decomposition is shape-directed rather than value-directed, so it
already applies; the work is the query surface and parameter plumbing.

## Composition with `Deferred<T>`

`Deferred<T>` composes with document storage, but the lowering differs
from the column-expanded case (where the driver omits the column from
the `SELECT` list). When the deferred field lives inside a document
column, the driver emits a path-exclusion expression instead:
`col - 'key'` / `col #- '{a,key}'` on PostgreSQL, `JSON_REMOVE` /
`json_remove` on MySQL / SQLite, an exclusion projection on MongoDB.
DynamoDB has no exclusion form; the driver enumerates the non-deferred
paths in a `ProjectionExpression`. Exclusion saves wire bytes, not
server-side read cost (PostgreSQL still detoasts the value; DynamoDB
still charges read capacity for the full item). The full deferred-loading
design lives with the partial-loading work; this document only commits
to the storage representations being deferred-loadable.

## Alternatives considered

**Whole-value equality at every tier (no decomposition).** Compare an
embed as one JSON / Map value everywhere. Rejected: it silently changes
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
4 KB condition-expression cap), it duplicates semantics the whole-value
form must implement anyway for non-constant comparisons, and native
structural equality already exists on three of four backends.

**SQLite text equality for the collection tier.**
`json_extract(...) = json(?)` is exact for Toasty-written rows (one
canonical encoder on both sides) but key-order sensitive for externally
written ones. Rejected as the shipping form because it would make SQLite
the one backend with divergent semantics; kept in mind as a fast path if
the `json_tree` form proves too slow, at which point the divergence
would need to be a documented capability, not a silent one.

**Rejecting unsupported drivers instead of in-memory fallback.** The
`.ilike()` precedent rejects, but that rule exists because backends
*disagree* on ilike's semantics. Value equality has one semantics, and
the in-memory evaluator implements it exactly, so transparent fallback
is the established pattern (it already backs post-filters on key-fetch
paths).

**External or adjacent enum tagging.** Serde's default external tagging
adds a nesting level per variant; DynamoDB's 32-level nesting cap makes
that a real budget. Internal tagging with `type` matches the most
common serde convention and keeps nesting flat.

## Open questions

- **Default `create_if_missing` for `stmt::patch`.** PostgreSQL's
  `jsonb_set` takes a flag; Mongo's `$set` always creates. True is more
  forgiving; false catches typos. Blocking acceptance.
- **Set semantics for `HashSet<scalar>` on PostgreSQL.** `text[]` has no
  uniqueness guarantee, and atomic in-place operations (`stmt::insert`,
  `stmt::extend`) modify the array server-side without loading the row,
  so they can introduce duplicates that violate `HashSet`'s invariant.
  Options: a CHECK constraint per HashSet field, a DOMAIN type, or
  compiling each set mutation to a set-aware form (conditional append).
  Blocking implementation of milestone 4.
- **`Set<T>` marker layout.** Confirm the trait layout (`Into<Set<T>>`
  blanket impls) and whether `Set<T>` carries uniqueness as a runtime
  invariant or a name-only convention. Blocking implementation of
  milestone 4.
- **`Partial<T>` representation and `partial!` mechanics.** The natural
  shape is `struct Partial<T> { expr: stmt::Expr, _p: PhantomData<T> }`,
  with `partial!` lowering struct-literal syntax to an `stmt::Record` of
  named values. Open: how much of the create-builder machinery it can
  reuse for field-name validation. Blocking implementation of
  milestone 5.
- **`json_tree` rendering under negation and disjunction.** The
  EXCEPT-pair is a pair of scalar subqueries; nesting it inside `OR` /
  `NOT` duplicates it. If that proves unacceptable, milestone 3 narrows
  to top-level conjuncts and other positions stay on the in-memory path.
  Blocking milestone 3 implementation, not acceptance.
- **`ne()` null semantics.** Whether `ne` over an embed with `Option`
  fields means strict negation of `eq` (absent counts as "not equal") on
  every backend, given SQL's three-valued logic. Blocking the follow-on,
  not this design.
- **Equality capability granularity.** One `document_value_eq` flag, or
  split per shape (embed vs. list vs. future map)? Start with one; split
  when a real driver needs the distinction. Deferrable.
- **Map keys containing `.`.** Mongo path notation uses `.` as a key
  separator. Blocking implementation for the Mongo driver; deferrable
  for the SQL-only case.
- **`HashMap` ordering.** PG `jsonb` sorts keys; SQLite preserves input
  order. Document the lack of ordering guarantee or normalize on
  encode? Deferrable.
- **Renaming document keys.** `#[document(rename = "...")]` on an embed
  field is the natural form (a `#[column]` rename is already rejected
  under document storage). Deferrable.
- **Backend-specific index modifiers.** PG opclass selection,
  partial-index conditions, Mongo wildcard projection, DynamoDB GSI
  projection attributes need a modifier syntax. Deferrable.

## Out of scope

- **Ordering comparisons on document values.** `lt` / `gt` on an embed
  or collection have no meaningful cross-backend semantics and are not
  planned.
- **Raw document path expressions.** A `path_match("$.a[*] ? (@.b > 1)")`
  escape hatch; postponed until the typed surface proves insufficient.
- **DynamoDB nested-path indexing.** Requires denormalizing to a
  top-level attribute; a `#[index(extract(...))]` form could automate it
  later.
- **Schema migrations for document shape changes.** Bulk
  read-modify-write; no special migration primitives here.
- **Full-text search over documents.** Separate roadmap item.
- **Server-side aggregation pipelines.** Covered by the broader
  aggregation design.
- **JSON Schema validation.** Separate feature; check constraints are
  already a roadmap item.
