# Document and Collection Fields

Subsumes the [JSON field queries roadmap entry](../roadmap.md#query-engine).

## Summary

Toasty picks the best per-backend storage representation for embedded
types and collection fields by default — column-expanded embeds on SQL
backends, flat-expanded top-level attributes on DynamoDB, BSON
sub-documents on MongoDB, `text[]` for `Vec<scalar>` on PostgreSQL,
typed Sets for `HashSet<scalar>` on DynamoDB, JSON wherever no native
option exists. `#[document]` is an explicit override that forces
document storage where the backend has a meaningful distinction. The
query API is the same regardless of storage; collection paths expose
`std`-aligned methods (`contains`, `is_superset`, `intersects`).
Updates target nested paths via `stmt::patch`, with array push/remove
and numeric increment for atomic in-place changes when the backend
supports it.

## Motivation

Toasty has no story for `Vec<T>` or `HashMap<String, T>` on a model
today, and no way to store an `#[derive(Embed)]` type as a single
document. Both come up constantly:

- Open-ended attributes — user preferences, feature flags per row,
  request metadata, audit context.
- Tag-like collections — labels, capabilities, role lists.
- Heterogeneous shapes that vary across rows and don't fit an enum.
- Lists of small structured items where one column per item is
  impossible (line items, change-log entries).

The existing `#[serialize(json)]` attribute stores any serde type as
opaque JSON text. It works, but Toasty cannot query into it, index
sub-paths, or patch a single field — every change is a read-modify-
write of the full value. PostgreSQL's native arrays and `jsonb`,
MongoDB's BSON, and DynamoDB's Map / List / typed-Set attributes all
expose rich operators that should be reachable from Toasty's typed
query API.

This design gives one user-facing query API spanning native and
document storage and across SQL, document, and key-value backends.
MongoDB support is aspirational — Toasty has no MongoDB driver yet —
but the API is designed so adding one does not require new
user-facing concepts. DynamoDB has stronger native document support
than the SQL backends in some places (atomic patch and atomic
increment at any depth) and weaker in others (no sub-document
containment, no element predicate, indexing only on top-level
attributes).

## User-facing API

### Storage selection at a glance

Toasty chooses storage per (backend, field type) by default. The query
API is identical across choices.

| Field type | PostgreSQL | MySQL | SQLite | MongoDB | DynamoDB |
|---|---|---|---|---|---|
| `#[derive(Embed)]` struct/enum | column-expanded | column-expanded | column-expanded | sub-document | flat-expanded |
| `Vec<scalar>` | `T[]` (e.g. `text[]`) | `JSON` | JSON1 | BSON array | List `L` |
| `Vec<struct>` | `jsonb` | `JSON` | JSON1 | BSON array | List `L` |
| `HashSet<scalar>` | `T[]` | `JSON` | JSON1 | BSON array | typed Set `SS`/`NS`/`BS` |
| `HashMap<String, T>` | `jsonb` | `JSON` | JSON1 | BSON sub-document | Map `M` |

DynamoDB defaults embeds to flat-expansion — one top-level attribute
per leaf field, named `preferences_theme` and so on, the same shape
as SQL columns. The reason: DynamoDB items are structurally closer to
SQL rows than to MongoDB documents (flat dictionaries, arbitrary
attribute names, GSI / LSI keys must be top-level scalars).
Flat-expansion makes any embed leaf indexable as a GSI / LSI key
directly. MongoDB defaults to sub-documents because its idiom,
tooling, and index model favor nesting; DynamoDB shares neither of
those traits.

`#[document]` overrides the default to document storage on backends
where there is a meaningful distinction. MongoDB stores everything
as BSON, so the override is a no-op there. On DynamoDB it forces a
Map (`M`) attribute over flat-expansion, or a Map / List over a typed
Set, making `#[document]` meaningful again. See
[Forcing document storage](#forcing-document-storage).

DynamoDB typed Sets (`SS`, `NS`, `BS`) only support string, numeric, and
binary element types respectively. `HashSet<bool>`, `HashSet<struct>`,
and other element types fall back to List `L` storage on DynamoDB.

### Embedded types

An `#[derive(Embed)]` field column-expands by default, the existing
behavior:

```rust
#[derive(toasty::Embed)]
struct UserPreferences {
    theme: String,
    notifications: NotificationSettings,
    locale: Option<String>,
}

#[derive(toasty::Embed)]
struct NotificationSettings {
    email: bool,
    push: bool,
}

#[derive(toasty::Model)]
struct User {
    #[key] #[auto]
    id: u64,

    preferences: UserPreferences,        // expands to preferences_theme, ...
}
```

#### Collections inside an embed

Storage selection applies recursively. Column-expansion continues
through nested embeds; collection fields stop the expansion at the
collection itself, which becomes one column with the same backend-
chosen storage it would have at the model root:

```rust
#[derive(toasty::Embed)]
struct UserPreferences {
    theme: String,                          // -> preferences_theme: TEXT
    tags: Vec<String>,                      // -> preferences_tags: text[] (PG) / jsonb (else)
    feature_flags: HashMap<String, bool>,   // -> preferences_feature_flags: jsonb
}
```

The path API works through the boundary unchanged:

```rust
User::all().filter(
    User::FIELDS.preferences().tags().contains("beta")
);
```

The collection's column inherits its native operators on PostgreSQL —
`text[]` for `Vec<scalar>` is GIN-indexable, supports `= ANY(col)` and
`@>`, and so on — so the only difference between a root-level and an
embed-nested collection is the column name.

#### Override: document storage for a single nested field

`#[document]` on a field inside an embed forces that one field to
document storage while the rest of the embed stays column-expanded.
Useful when most fields should live in their own columns (for
indexing, sort, or schema clarity) but one nested embed is large or
irregular enough that exploding it into per-leaf columns is
unhelpful:

```rust
#[derive(toasty::Embed)]
struct Profile {
    name: String,                            // column: profile_name
    age: u32,                                // column: profile_age

    #[document]
    layout: LayoutPreferences,               // single jsonb column: profile_layout
}

#[derive(toasty::Embed)]
struct LayoutPreferences {
    sidebar_width: u32,
    panel_order: Vec<String>,
    pinned_widgets: Vec<WidgetConfig>,
    // ...many more fields the user doesn't query individually
}
```

Without the override, `LayoutPreferences` would expand to
`profile_layout_sidebar_width`, `profile_layout_panel_order`,
`profile_layout_pinned_widgets`, and so on. With `#[document]` the
whole sub-tree becomes one `jsonb` column on PostgreSQL. The path
API still works (`User::FIELDS.profile().layout().sidebar_width()`
filters the same way); the difference is column count, indexability
of individual leaves, and storage layout.

### Collections at the model

`Vec<T>`, `HashMap<String, T>`, and `BTreeMap<String, T>` work with no
attribute. Storage is backend-chosen:

```rust
#[derive(toasty::Model)]
struct User {
    #[key] #[auto]
    id: u64,

    tags: Vec<String>,                   // text[] on PG, JSON elsewhere
    metadata: HashMap<String, String>,   // jsonb on PG, BSON sub-doc on Mongo
}
```

`User::FIELDS.tags()` is a typed path of element type `String` exposing
methods that mirror `std`:

- `.contains(impl Into<Expr<T>>) -> Expr<bool>` — the array contains
  this element. (`Vec::contains`.)
- `.is_superset(impl Into<Set<T>>) -> Expr<bool>` — the array
  contains every element of the right-hand set.
  (`HashSet::is_superset`.)
- `.intersects(impl Into<Set<T>>) -> Expr<bool>` — the array shares
  at least one element with the right-hand set. (Negation of
  `HashSet::is_disjoint`; no positive `std` form exists.)
- `.len() -> Expr<usize>` — the array length as a typed numeric
  expression.
- `.is_empty() -> Expr<bool>` — equivalent to `.len().eq(0)`.

`User::FIELDS.metadata()` is a typed path of key type `String` and
value type `String`:

- `.contains_key(impl Into<Expr<K>>) -> Expr<bool>` — the map has
  this key. (`HashMap::contains_key`.)
- `.keys()` — a set view exposing the same set methods as `Vec`
  paths (`.contains`, `.is_superset`, `.intersects`).
- `.values()` — an iterable view supporting `.any(|v| …)` and
  `.all(|v| …)` (Tier 2 below).
- `.len()`, `.is_empty()`.

`Set<T>` is a marker for set-shaped expressions, analogous to the
existing `List<T>` marker for ordered collections. Common Rust types
implement the relevant `Into` impl so values pass through directly:
`Vec<T>`, `&[T]`, `[T; N]`, `HashSet<T>`, and `BTreeSet<T>` all
satisfy `Into<Set<T>>` and `Into<List<T>>` where applicable.

### Forcing document storage

`#[document]` forces a field to document storage. Use it for:

- An embed type that should be one column instead of column-expanded.
- A collection that needs document encoding even on backends with a
  native specialized type — useful when the same model is shared
  across backends and you want uniform query semantics, or when the
  element shape is heterogeneous.

```rust
#[derive(toasty::Model)]
struct User {
    #[key] #[auto]
    id: u64,

    #[document]
    preferences: UserPreferences,        // single jsonb column

    #[document]
    tags: Vec<String>,                   // jsonb on PG (forced), JSON elsewhere
}
```

Accepted field types:

- Any `#[derive(Embed)]` struct or enum.
- `Vec<T>`, `HashSet<T>`, `BTreeSet<T>` of an encodable `T`.
- `HashMap<String, T>`, `BTreeMap<String, T>`.
- `Option<T>` of any of the above.
- Bare scalars (`i64`, `String`, `bool`, etc.) — useful when the
  column must accept multiple value types.

### Storage modifiers

```rust
#[document]            // jsonb on PG, BSON sub-doc on Mongo, JSON on MySQL/SQLite, M on DynamoDB
preferences: UserPreferences,

#[document(text)]      // PostgreSQL `json` (text). Other backends ignore the modifier.
audit_blob: AuditBlob,
```

Reach for `#[document(text)]` only when exact-byte preservation
matters — audit trails, JSON received from a third party that you
re-emit verbatim.

### Tier 1: equality, range, and null on a nested field

Path access works for both column-expanded and document-stored embeds:

```rust
User::all().filter(
    User::FIELDS.preferences().theme().eq("dark")
);

User::all().filter(
    User::FIELDS.preferences().notifications().email().eq(true)
);

User::all().filter(
    User::FIELDS.preferences().locale().is_none()
);
```

`is_none()` matches both an absent key and an explicit JSON null; see
[Behavior](#behavior) for the distinction.

### Tier 1: sub-document containment

Containment asks "does the document contain this shape, anywhere it
matches?" Build a partial value of the embed's type with the
`partial!` macro:

```rust
User::all().filter(
    User::FIELDS.preferences().contains(toasty::partial!({
        theme: "dark",
    }))
);
```

The signature is `.contains(impl Into<Expr<Partial<T>>>) -> Expr<bool>`,
where `T` is the embed type. `Partial<T>` is a generic type carrying
a list of named field assignments; the `partial!` macro produces one
from struct-literal syntax. Field names not in `{ ... }` are absent
from the predicate, so `partial!({ theme: "dark" })` matches any row
whose `preferences.theme` is `"dark"`, regardless of other fields.

Nested partial values work the same way:

```rust
User::all().filter(
    User::FIELDS.preferences().contains(toasty::partial!({
        notifications: { email: true },
    }))
);
```

The `{ … }` body inside a field position produces a nested
`Partial<NotificationSettings>` that participates in the deep match.

Compile-time field-name validation follows the same pattern as
[the static assertions for `create!`](static-assertions-create-macro.md):
the macro emits a const-evaluation block bounded on a hidden trait
that resolves the target type at monomorphization time, and the
const panics if a field name is unknown. The compile-time check is a
DX nicety, not a correctness requirement — the runtime path validates
keys against the schema as well, so a v1 implementation can ship the
runtime check first and add the compile-time one later.

Containment is a document-storage feature; the query engine rejects
`.contains(...)` on a column-expanded embed with a clear error
pointing at `#[document]`.

### Tier 1: collection predicates

The collection methods work the same for native-array and document-
stored collections:

```rust
User::all().filter(User::FIELDS.tags().contains("admin"));
User::all().filter(User::FIELDS.tags().is_superset(["admin", "verified"]));
User::all().filter(User::FIELDS.tags().intersects(["admin", "moderator"]));
User::all().filter(User::FIELDS.tags().len().gt(3));

User::all().filter(User::FIELDS.metadata().contains_key("source"));
User::all().filter(
    User::FIELDS.metadata().keys().is_superset(["source", "version"])
);
```

The lowering picks the right operator per storage: `tags.contains("x")`
becomes `'x' = ANY(tags)` against `text[]` and `tags @> '["x"]'::jsonb`
against `jsonb`, both indexable.

### Tier 1: full-value replacement

```rust
user.update()
    .preferences(new_preferences)
    .exec(&mut db)
    .await?;
```

Whole-value writes work on every backend with no special operator support.

### Tier 1: nested patch via `stmt::patch`

`stmt::patch(path: Path<T>, value: impl Into<Expr<T>>)` updates one
field inside a document and leaves the rest unchanged. It applies to
document-stored embeds:

```rust
user.update()
    .preferences(stmt::patch(
        UserPreferences::FIELDS.theme(),
        "light",
    ))
    .exec(&mut db)
    .await?;
```

On column-expanded embeds, the same `stmt::patch` call is already
supported and translates to a column-level update.

### Tier 1: numeric increment

`stmt::increment(path: Path<T>, by: impl Into<Expr<T>>)` where `T` is
a numeric type:

```rust
user.update()
    .stats(stmt::increment(
        UserStats::FIELDS.login_count(),
        1,
    ))
    .exec(&mut db)
    .await?;
```

Atomic on backends that support it (PostgreSQL `jsonb_set` with a
computed expression for document fields, native column update for
column-expanded fields, MongoDB `$inc`, DynamoDB `ADD`); falls back
to read-modify-write otherwise.

### Tier 1: array push and remove

```rust
user.update().tags(stmt::push("admin")).exec(&mut db).await?;
user.update().tags(stmt::push_all(["admin", "verified"])).exec(&mut db).await?;
user.update().tags(stmt::remove_eq("guest")).exec(&mut db).await?;
```

The signatures:

- `stmt::push(impl Into<Expr<T>>)` — append one element.
- `stmt::push_all(impl Into<List<T>>)` — append multiple, in order.
- `stmt::remove_eq(impl Into<Expr<T>>)` — remove every element equal
  to the value.
- `stmt::remove_at(impl Into<Expr<usize>>)` — remove the element at
  this index (Tier 2 below).

The lowering uses native array operations (`array_append`,
`array_remove`) on PostgreSQL native arrays and document operations
on document-stored arrays.

### Tier 2: array element predicates

```rust
Order::all().filter(Order::FIELDS.line_items().any(|i|
    i.product_id().eq(123).and(i.quantity().gt(0))
));

Order::all().filter(Order::FIELDS.line_items().all(|i|
    i.in_stock().eq(true)
));
```

`any` is supported by every target backend (PG `jsonpath` predicate or
`EXISTS` over `jsonb_array_elements`, or `unnest` over `T[]`; Mongo
`$elemMatch`). `all` is supported on Mongo natively and via
`NOT any(NOT pred)` on PG. DynamoDB falls back to client-side
filtering.

### Tier 2: predicates over keys and values

For richer predicates over the entries of a map:

```rust
User::all().filter(
    User::FIELDS.metadata().keys().any(|k| k.starts_with("internal_"))
);
User::all().filter(
    User::FIELDS.metadata().values().any(|v| v.eq("ok"))
);
```

### Tier 2: removing a key or array element

```rust
user.update()
    .preferences(stmt::unset(UserPreferences::FIELDS.locale()))
    .exec(&mut db)
    .await?;

user.update()
    .tags(stmt::remove_at(2))
    .exec(&mut db)
    .await?;
```

`stmt::remove_at` matches `Vec::remove`. Out-of-bounds is a no-op
rather than an error, since per-row failure semantics on a bulk update
are rarely useful.

### Indexes

Index syntax follows the field's storage representation. The same
`#[index(...)]` attribute used for column indexes covers document and
array fields:

```rust
#[index(gin(tags))]                                   // PG GIN on text[] or jsonb
#[index(document_path(preferences => theme))]         // expression index on a single path
```

`gin(...)` produces a GIN index appropriate for the column's storage:
`array_ops` on `text[]`, `jsonb_ops` on `jsonb`, no-op on backends
without GIN. `document_path(...)` produces an expression index on a
single path within the document (PG B-tree, MongoDB path index).
Toasty rejects an index form that does not apply to the field's
storage with a clear error.

## Behavior

**Storage selection.** Toasty resolves the storage representation per
(backend, field type) at schema build time. The choice is observable
through the column type but not through the query API:

- Embed types column-expand on SQL backends, flat-expand to top-level
  attributes on DynamoDB, and become sub-documents on MongoDB.
- `Vec<scalar>` uses the backend's native array type if one exists
  (`text[]`, `int[]`, etc. on PostgreSQL; BSON array on MongoDB; List
  `L` on DynamoDB) and JSON otherwise.
- `HashSet<scalar>` uses a typed Set on DynamoDB (`SS` / `NS` / `BS`
  per element type) and otherwise the same representation as
  `Vec<scalar>`.
- `Vec<struct>` and all map types use document storage (JSON on SQL
  backends, BSON / Map / sub-document on document backends).
- **Recursion through embeds.** Selection applies to every field on
  the way down. A nested embed continues to expand; a nested
  collection stops the expansion and becomes one slot with the same
  storage it would receive at the root. The slot name is the
  underscore-joined path (`preferences_tags`) on SQL and DynamoDB,
  the dotted path (`preferences.tags`) on MongoDB.
- `#[document]` overrides the default to document storage for any
  field — at the root or anywhere inside an expanded embed. Marking
  one nested field forces that field alone; marking the parent embed
  forces the whole sub-tree. On MongoDB the override is a no-op
  since BSON is the only encoding; on DynamoDB it forces a Map (`M`)
  over flat-expansion, or a Map / List over a typed Set.
- `#[document(text)]` further selects PG text `json` over `jsonb`;
  ignored on other backends.

**Encoding.** Toasty serializes to the document encoding (JSON on SQL
and SQLite, BSON on MongoDB, DynamoDB attribute trees on DynamoDB)
using the same field-by-field representation it would use for
column-expanded embeds, packed into one document. Enum discriminators
encode as a `__type` key by default; configurable per the open
question below. Numeric types preserve Rust width where the backend
supports it (Mongo Int32/Int64; PG `jsonb` numeric). Floating-point
NaN and infinity are rejected at encode time — JSON has no
representation for them.

**Column-rename attributes on document-stored embeds.** A
`#[column("name")]` annotation on a field of an embed type used as
`#[document]` is an error at schema build time. The annotation
renames a SQL column suffix in the column-expanded case; under
`#[document]` there is no column to rename, and document keys come
from the Rust field name. Renaming document keys is a future feature
(likely `#[document(rename = "...")]`).

**Null vs missing key.** `Option<T>` writes nothing for `None` and a
document value for `Some`. Reading distinguishes:

- Absent key → `None`.
- Explicit JSON `null` → `None`, with `#[document(strict_nulls)]`
  opting into an error instead.

`is_none()` matches both. `is_absent()` and `is_null()` match only one
each.

**Patch semantics.** A `stmt::patch(path, value)` writes `value` at
`path`, creating intermediate objects as needed. A patch that walks
through a field whose current type is incompatible (e.g. patching
`notifications.email` when `notifications` is currently a JSON array)
returns a runtime error on the affected row.

**Array writes.** `stmt::push` appends, creating the array if absent.
`stmt::remove_eq` removes every matching element. `stmt::remove_at`
removes a single index; out-of-bounds is a no-op.

**Concurrent updates.** PostgreSQL `jsonb_set` rewrites the entire
document; concurrent patches to disjoint paths within one `jsonb`
column are not independent — last write wins. PostgreSQL native
arrays have similar whole-column semantics. MongoDB `$set` and
DynamoDB `SET path = …` on disjoint paths are independent. Code
that depends on per-path atomicity should not rely on it on SQL
backends.

## Backend mapping

The query engine emits different operators depending on storage. The
table shows both forms where they differ; backends without a column
imply the document form is used regardless.

| Operation | PG native (`T[]`) | PG `jsonb` | MongoDB | SQLite (JSON1) | MySQL | DynamoDB |
|---|---|---|---|---|---|---|
| Path equality | n/a | `col->'a'->>'b' = …` | `{"a.b": …}` | `json_extract` | `JSON_EXTRACT` | `path = :v` |
| Containment | n/a | `@>` | structural match | `json_each` + filter | `JSON_CONTAINS` | AND of path equalities |
| `contains_key` | n/a | `?` | `$exists` | `json_extract IS NOT NULL` | `JSON_CONTAINS_PATH` | `attribute_exists(path)` |
| `contains` (array) | `= ANY(col)` | `@>` | `{arr: v}` | `json_each` | `JSON_CONTAINS` | `contains(path, :v)` |
| `is_superset` | `@>` | `@>` | `$all` | `json_each` | `JSON_CONTAINS` | AND of `contains()` |
| `intersects` | `&&` | `?\|` | `$in` (per-element) | `json_each` | `JSON_OVERLAPS` | OR of `contains()` |
| `len` | `cardinality` | `jsonb_array_length` | `$size` | `json_array_length` | `JSON_LENGTH` | `size(path)` |
| `any` predicate | `EXISTS unnest` | `EXISTS jsonb_array_elements` | `$elemMatch` | `EXISTS json_each` | `JSON_TABLE` | client-side filter |
| Patch one path | column update | `jsonb_set` | `$set` | `json_set` | `JSON_SET` | `SET path = :v` |
| Increment | column update | `jsonb_set` with cast | `$inc` | `json_set` arith | `JSON_SET` arith | `ADD path :n` |
| `push` | `array_append` | `\|\|` | `$push` | `json_insert` | `JSON_ARRAY_APPEND` | `SET path = list_append(path, :v)` |
| `remove_eq` | `array_remove` | `jsonb_set` minus filter | `$pull` | rewrite | rewrite | `DELETE path :s` (typed Set) / RMW |
| `remove_at` | array slicing | `jsonb_path` minus | `$unset` + `$pull` | rewrite | `JSON_REMOVE` | `REMOVE path[i]` |
| `unset` (key) | n/a | `-` | `$unset` | `json_remove` | `JSON_REMOVE` | `REMOVE path.key` |

### Future MongoDB gaps

Items the API expresses cleanly but the eventual Mongo driver will
need to work through:

- **BSON-only types in `#[document]` embeds.** `ObjectId`, `Date`,
  `Decimal128`, and `UUID` have no JSON representation. A Mongo-
  backed model must be able to declare these types in an embed (e.g.
  `created_at: bson::DateTime`) and have them encoded as BSON
  natively. Until then, `#[document]` embeds containing such types
  are rejected at schema build time.
- **Positional array operators.** Mongo's `$[<id>]` and `arrayFilters`
  let `$set` target specific elements within nested arrays atomically.
  Toasty's `stmt::patch` over an array path needs to compile to those
  operators on Mongo to retain atomicity; on PG it falls back to
  whole-document rewrite anyway.
- **Wildcard / multikey index DDL.** Index forms above cover the
  common cases, but Mongo's compound multikey rules and wildcard
  projection do not have a one-line DDL today.
- **Sharding by document-path key.** Mongo shard keys can be document
  paths. Toasty's key model is single-field; this is out of scope for
  v1 and may need a broader composite-key story.
- **Map keys containing `.`.** Mongo path notation uses `.` as a key
  separator; map keys containing literal dots need escaping or
  rejection. Decided per the open question below.

### DynamoDB constraints

DynamoDB has stronger native document support than the SQL backends in
several places (atomic patch, atomic increment, atomic push, atomic
remove-at), but a few gaps stand out:

- **No sub-document containment.** DynamoDB has no `@>`-equivalent.
  `.contains(partial)` lowers to an AND of explicit path equalities,
  which works but does not match the semantics of "any matching
  shape anywhere in the document." Documented and consistent; users
  who need deep structural match fall back to load-and-filter.
- **No element predicate.** `.any(|i| ...)` and `.all(|i| ...)` over
  a list cannot be evaluated server-side — DynamoDB's filter
  expression language has no per-element predicate. Toasty falls
  back to client-side filtering after the read.
- **`remove_eq` on Lists.** Atomic removal of every element equal to
  a value works for typed Sets via `DELETE path :s`. For Lists (the
  default for `Vec<T>`), Toasty falls back to read-modify-write.
  `HashSet<scalar>` users get the atomic path automatically through
  the typed Set storage default.
- **Indexing nested paths.** GSI / LSI keys must be top-level scalar
  attributes. Flat-expanded embed leaves are top-level attributes
  and can be GSI / LSI keys directly. Document-stored fields
  (`#[document]`-marked, Vec, Map, typed Set) are not indexable
  except by denormalizing the value to a separate top-level
  attribute. Out of scope for v1; a `#[index(extract(...))]` form
  could automate the denormalization case later.
- **Item size cap.** 400 KB per item, smaller than PG TOAST and
  MongoDB's 16 MB. Tighter constraint on what fits in one document.
- **Filter expressions don't reduce IO.** DynamoDB filters apply
  after `Query` / `Scan` reads; capacity is consumed for everything
  matched by the key conditions. Document-path predicates that lower
  to filter expressions still cost the full scan.

## Edge cases

- **Mixed-type values at a path.** Filtering
  `User::FIELDS.metadata().get("count").eq(5)` matches when the value
  at `count` is the JSON number 5; it does not coerce the JSON string
  `"5"`. Strict typing is the default.
- **Empty PG arrays.** `array_length(col, 1)` returns `NULL` for an
  empty array; Toasty's `.len()` lowers to `cardinality(col)` instead,
  which returns 0.
- **Empty document vs missing column.** `NOT NULL` document-stored
  fields default to `{}` (objects), `[]` (arrays), or the embed's
  default; `Option<T>` allows SQL `NULL`. Native-array fields default
  to `'{}'` (an empty array literal in PG).
- **Document size limits.** PostgreSQL TOAST caps individual values
  near 1 GB; MongoDB caps documents at 16 MB; DynamoDB caps items at
  400 KB. Toasty does not enforce a smaller limit. Inserts exceeding
  the backend limit surface a driver error.
- **DynamoDB typed-Set element types.** Typed Sets only support
  string, numeric, and binary element types. A model field of
  `HashSet<bool>` or `HashSet<MyEnum>` falls back to List `L` storage
  on DynamoDB and loses the atomic `ADD` / `DELETE` path. The
  fallback is silent; users who care about atomicity should pick a
  scalar element type.
- **Floating-point edge values.** NaN and infinity are rejected at
  encode time. Negative zero round-trips as zero on PostgreSQL `jsonb`
  (it stores `numeric`).
- **Reading legacy data.** A row whose document does not match the
  current schema (extra keys, missing keys, wrong type at a path)
  deserializes field-by-field; missing required fields surface as
  decode errors, extra keys are dropped silently. A future
  `#[document(strict)]` modifier could opt into rejecting extra keys.
- **Schema drift between writes.** A patch onto a row whose document
  was written by an older or external writer may produce surprising
  shapes (e.g. a key that was a string is now an object). Toasty does
  not validate the existing document before patching.

## Driver integration

**New schema artifacts.** Drivers see two new column-type families:

- `ColumnType::Array(elem)` — PostgreSQL native array of `elem`.
  Other SQL drivers reject this and the planner picks document
  storage instead at schema-build time.
- `ColumnType::Document { binary: bool }` — the dialect's document
  type. `binary: true` maps to `jsonb` / `JSON` / JSON1 / BSON / `M`
  as appropriate; `binary: false` selects PG text `json` and is
  ignored elsewhere.

**New operations.** SQL drivers gain new statement nodes that the SQL
serializer renders to dialect-specific operators. The same nodes
lower differently against `Array` vs `Document` column types:

- `stmt::Expr::ArrayContains`, `ArrayIsSuperset`, `ArrayIntersects`,
  `ArrayLength`, `ArrayAny { var, body }`, `ArrayAll { var, body }`.
- `stmt::Expr::DocumentPath { value, path }`, `DocumentContains`,
  `DocumentContainsKey`.
- Update RHS forms: `stmt::Assign::ArrayAppend`, `ArrayRemoveEq`,
  `ArrayRemoveAt`; `DocumentSet`, `DocumentInc`, `DocumentUnset`.

Each is gated behind a capability flag. The planner reads
capabilities to decide whether to push the operator to the driver or
fall back to an in-memory implementation. Drivers that implement none
of the new capabilities still work — every document or array
predicate compiles to load-and-filter, every update compiles to
read-modify-write — they just lose the per-operator optimizations.

**MongoDB driver (future).** The driver compiles statement nodes
directly to its query and update document forms; SQL serialization
does not apply. The `Capability::DocumentInPlaceAtomic` flag exposes
whether disjoint-path patches are independent.

**DynamoDB driver.** The existing driver gains:

- A schema-build pass that picks flat-expansion, `M`, `L`, or
  typed-Set storage per field type as described in the storage
  selection table. Embeds default to flat-expansion (top-level
  attributes); `#[document]` opts into Map.
- Compilation from `stmt::Expr` nodes to DynamoDB condition
  expressions, including AND-of-`contains` for `is_superset` and
  OR-of-`contains` for `intersects`.
- Compilation from `stmt::Assign` nodes to update expressions
  (`SET path = …`, `ADD path :n`, `REMOVE path[i]`, etc.).
- Capability flags advertise native support for all writes except
  `DocumentContainment`, `DocumentAnyPredicate`, and
  `DocumentRemoveEqOnList`, which fall back to load-and-filter or
  read-modify-write.

**Out-of-tree drivers.** Existing drivers compile unchanged. New
operations are gated behind capability flags; absent flags fall back
to load-and-rewrite paths through the existing `QuerySql` and
`Insert` operations.

## Alternatives considered

**Naming the attribute after the encoding (`#[json]`).** The first
draft of this design used `#[json]`, which is the most-recognized
term across backends and matched the original "JSON field queries"
roadmap framing. Rejected once the design grew to cover non-JSON
storage (PostgreSQL native arrays, DynamoDB Map / List / typed Sets):
`#[document]` is encoding-neutral and accurately describes the user
intent ("store as one document") on every backend, including those
where the encoding is not JSON.

**DynamoDB embeds default to Map (`M`).** An earlier draft defaulted
DynamoDB embeds to Map storage, mirroring MongoDB sub-documents
under the assumption that "document backends" should treat embeds
as documents. Rejected: DynamoDB items are structurally closer to
SQL rows than to MongoDB documents — flat dictionaries with
arbitrary attribute names, where GSI / LSI keys must be top-level
scalars. The same reasoning that justifies column-expansion on SQL
(indexability, simpler filter expressions, no idiom mismatch)
applies to DynamoDB. Mongo really is different because its tooling,
BSON encoding, and index model favor sub-documents; DynamoDB shares
none of those traits.

**Always force document storage; no native arrays in v1.** Skip the
backend-dependent default and require `#[document]` for any
collection. Rejected: backend-dependent storage matches Toasty's
existing pattern for embedded enums (where the discriminator column
type is also backend-chosen), keeps PostgreSQL users on native arrays
where they're faster and smaller, and avoids forcing an attribute
that does nothing on backends without an alternative.

**Two attributes, `#[array]` and `#[document]`.** Surface explicit
storage choice for both. Rejected: native arrays are the natural
default for the field types where they apply; an explicit `#[array]`
would only exist to mean "the default." `#[document]` is the only
choice that needs its own attribute because it's the override.

**Keep `#[serialize(json)]` and add no new attribute.** Reuse the
existing opaque-blob attribute for the queryable case as well,
distinguishing by type (Embed vs serde). Rejected because the storage
and query capabilities differ enough that two attributes are clearer
than one overloaded one. `#[serialize(json)]` remains for cases where
the user wants a serde-only escape hatch with no querying.

**Always store embeds as documents; no flag.** Removes the choice.
Rejected: column-expanded embeds give per-field indexes, smaller
rows, and existing SQL-tuning techniques that document storage
forecloses. The choice is load-bearing.

**Generated `{Type}Partial` companion struct for containment.** The
first draft of the containment API generated a companion struct
(e.g. `UserPreferencesPartial`) per `#[document]` embed, with every
field wrapped in `Option`. Rejected: long type names cluttered call
sites, the `..Default::default()` boilerplate appeared at every
call, and codegen grew with every embed. The single generic
`Partial<T>` plus a `partial!` macro is shorter at the call site,
generates less code, and keeps one type for every embed. The
compile-time validation that prevents typos in field names rides on
the same monomorphization trick used by the `create!` static
assertions.

**`#[column(type = json)]` instead of `#[document]`.** Reuse the
existing `#[column(type = ...)]` mechanism. Rejected: `#[column]`
takes a column-type name, and "json" leaks the encoding. A future
`#[column(type = jsonb)]` for explicit PG-specific storage stays
available.

**Document-collection API distinct from Embed.** A separate, Mongo-
flavored "collection of documents" surface alongside the relational
one. Rejected: two parallel modeling APIs is more surface than the
value warrants when embed already covers nested data.

**Naming after Mongo (`has`, `has_all`, `has_any`).** Rejected for
Rust idiom: `Vec::contains`, `HashSet::is_superset`, and the
`intersects` form (negation of `is_disjoint`) read more naturally to
Rust users.

## Open questions

- **Default `create_if_missing` for `stmt::patch`.** PostgreSQL's
  `jsonb_set` takes a flag; Mongo's `$set` always creates. True is
  more forgiving; false catches typos. Blocking acceptance.
- **Discriminator key name for enums.** `__type`? `$type`? Configurable
  per enum? Blocking implementation.
- **`Vec<struct>` storage on PostgreSQL.** PG composite types and
  arrays-of-composites work but the operators are weak and the
  encoding is awkward. The default in the table above is `jsonb`;
  confirm or revisit. Deferrable.
- **Set semantics for `HashSet<scalar>` on PostgreSQL.** PG `text[]`
  has no uniqueness guarantee, and atomic in-place operations
  (`stmt::push` and friends) modify the array server-side without
  loading the row, so they can introduce duplicates that violate
  `HashSet`'s invariant. Options include a CHECK constraint emitted
  per HashSet field, a DOMAIN type, or compiling each set-modifying
  operation to its set-aware equivalent (conditional append, or an
  `array_agg(DISTINCT …)` rewrite). Affects every write path that
  touches a HashSet field. Blocking implementation.
- **Map keys containing `.`.** Mongo path notation uses `.` as a key
  separator; allowing arbitrary string keys requires escaping on
  encode or rejection. Blocking implementation for the Mongo driver;
  deferrable for v1 PG-only.
- **`HashMap` ordering.** PG `jsonb` sorts keys; SQLite preserves
  input order; Mongo preserves input order. Document the lack of
  ordering guarantee or normalize on encode? Deferrable.
- **Index DDL syntax.** The `gin(...)` and `document_path(...)` forms
  above are a starting point; they may want subkey selection,
  opclass selection on PG, and partial-index conditions. Deferrable.
- **Renaming document keys.** `#[document(rename = "...")]` on an
  embed field is the natural form. Deferrable.
- **`Set<T>` marker type.** This design assumes a `Set<T>` marker
  alongside the existing `List<T>` to type the right-hand side of
  set predicates (`.is_superset`, `.intersects`). Confirm the trait
  layout (`Into<Set<T>>` blanket impls for `Vec<T>`, `&[T]`,
  `[T; N]`, `HashSet<T>`, `BTreeSet<T>`) and whether `Set<T>` should
  carry uniqueness as a runtime invariant or a name-only convention.
  Blocking implementation.

## Capability matrix

A high-level view of what each backend supports natively, what Toasty
emulates by composing native primitives, and what falls back to
client-side handling. The user-facing API is the same in every column;
the matrix captures only the cost and atomicity differences.

| Capability | PostgreSQL | MySQL | SQLite | MongoDB | DynamoDB |
|---|---|---|---|---|---|
| Path equality | native | native | native | native | native |
| Sub-document containment | native (`@>`) | native (`JSON_CONTAINS`) | compound | native | compound |
| Key existence | native | native | native | native | native |
| Array `contains` | native | native | native | native | native |
| Array `is_superset` | native | native | compound | native | compound |
| Array `intersects` | native | native | compound | native | compound |
| `len`, `is_empty` | native | native | native | native | native |
| `any` / `all` element predicate | native | native | native | native | client-side |
| Whole-value replace | native | native | native | native | native |
| `stmt::patch` (nested) | native | native | native (rewrite) | native (atomic) | native (atomic) |
| `stmt::increment` (nested) | native | native | native (rewrite) | native (atomic) | native (atomic) |
| `stmt::push` / `push_all` | native | native | native (rewrite) | native (atomic) | native (atomic) |
| `stmt::remove_eq` | native (`text[]`) / RMW (`jsonb`) | RMW | RMW | native (atomic) | native on typed Set / RMW on List |
| `stmt::remove_at` | native | native | native (rewrite) | native (atomic) | native (atomic) |
| `stmt::unset` | native | native | native (rewrite) | native (atomic) | native (atomic) |
| GIN / wildcard index | ✓ | — | — | ✓ | — |
| Path expression index | ✓ (B-tree) | partial | — | ✓ | denormalize to top-level attr |
| Disjoint-path concurrent writes | last-write-wins | last-write-wins | last-write-wins | independent | independent |

Legend:

- **native** — direct server-side operator, one round trip.
- **compound** — Toasty composes from native primitives, server-side,
  one round trip but more expressions evaluated.
- **RMW** — read-modify-write. Toasty does this transparently but it
  takes two round trips and is not atomic across concurrent writers
  unless the user wraps it in a transaction or condition.
- **client-side** — Toasty fetches and filters in process. Same
  number of round trips as a normal read but with more data over the
  wire.
- **rewrite** — operation rewrites the whole document column in one
  statement; transactional but not concurrency-safe across writers.

## Composition with `#[deferred]`

`#[deferred]` (planned, see [Partial model loading](../roadmap.md#schema--types)
on the roadmap) marks a field as not loaded by default; callers opt
in to retrieve it. The semantic is "load everything except deferred
fields." It composes cleanly with the storage choices in this
design, but the lowering depends on where the deferred field lives.

**Column-expanded fields.** The driver omits the deferred column
from the `SELECT` list. The existing column-projection path covers
this; nothing new is needed for document storage.

**Document-stored fields with deferred siblings.** When a deferred
field lives inside a document-stored embed — or when the document
column itself contains a deferred sub-path — the driver emits a
path-exclusion expression on the document column:

| Backend | Exclusion form |
|---|---|
| PostgreSQL `jsonb` | `col - 'deferred_key'` (top level) or `col #- '{a,deferred_key}'` (nested) |
| MySQL JSON | `JSON_REMOVE(col, '$.deferred_key')` |
| SQLite JSON1 | `json_remove(col, '$.deferred_key')` |
| MongoDB | exclusion projection: `find({}, { "path.to.deferred": 0 })` |
| DynamoDB | no native exclusion; driver emits `ProjectionExpression` listing every non-deferred path |

DynamoDB is the only backend that requires the driver to enumerate
the inclusion set rather than the exclusion set. Toasty knows the
schema, so this is a code-generation concern, not user-facing.

**Cost notes.** Deferred loading saves bandwidth in every case but
the per-backend cost reductions differ:

- PostgreSQL `jsonb` exclusion does not skip TOAST decompression;
  the row read cost is unchanged. The wire saving still matters
  when the deferred field is the reason the document is large.
- MongoDB exclusion projection saves wire bytes; the server still
  loads the full BSON document into memory.
- DynamoDB read capacity is charged per the full item size
  regardless of `ProjectionExpression`. Deferred loading saves
  parsing and bandwidth, not RCU.

The full design for deferred loading lives with the partial-loading
work; this document only commits to the storage representations
being deferred-loadable.

## Out of scope

- **Raw document path expressions.** A `path_match("$.a[*] ? (@.b > 1)")`
  escape hatch for queries the typed accessors cannot express.
  Defer until the typed surface proves insufficient.
- **DynamoDB nested-path indexing.** Indexing a value buried in a
  Map requires denormalizing it to a top-level attribute. A
  `#[index(extract(path))]` form could automate this; out of scope
  for v1.
- **Schema migrations for nested document shape changes.** Migrating
  a field from string to object across all rows is a bulk read-
  modify-write; no special migration primitives in this design.
- **Full-text search over documents.** Tracked as a separate roadmap
  item.
- **Server-side aggregation pipelines.** Mongo's `$group` /
  `$lookup` and PG's `jsonb_agg` are aggregation features; covered
  by the broader aggregation design.
- **JSON Schema validation.** Per-field structural validation is a
  separate feature; check constraints already exist as a roadmap item.
