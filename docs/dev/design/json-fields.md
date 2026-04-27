# JSON and JSONB Fields

Subsumes the [JSON field queries roadmap entry](../roadmap.md#query-engine).

## Summary

`#[json]` on a field stores an embedded type as a single JSON document
instead of expanding it into per-field columns. The same path-based query
API used for column-expanded embeds works on JSON fields, plus operators
specific to documents (containment, key existence, set predicates over
arrays). Updates target nested paths via `stmt::patch`, with array
push/remove and numeric increment for atomic in-place changes. Storage
defaults to `jsonb` on PostgreSQL; `#[json(text)]` opts into text JSON.

## Motivation

Users already model nested data with `#[derive(Embed)]`, but every embed
field expands to one column per leaf. That's the right default for narrow,
stable shapes. It's the wrong default for:

- Open-ended attributes — user preferences, feature flags per row,
  request metadata, audit context.
- Heterogeneous shapes that vary across rows and don't fit an enum.
- Lists of small structured items where one column per item is impossible
  (line items, tags-with-attributes, change-log entries).

The existing `#[serialize(json)]` attribute stores any serde type as opaque
JSON text. It works, but Toasty cannot query into it, index sub-paths, or
patch a single field — every change is a read-modify-write of the full
value. PostgreSQL's `jsonb` and MongoDB's BSON both expose rich
sub-document operators that should be reachable from Toasty's typed
query API.

This design gives one column-shape and one query API spanning both engines.
MongoDB support is aspirational — Toasty has no MongoDB driver yet — but
the API is designed so that adding one does not require new user-facing
concepts.

## User-facing API

Capabilities are presented in roughly the order they appear in real
applications. Tier 1 is the v1 surface; Tier 2 composes on top.

### Declaring a JSON field

Annotate a field whose type is `#[derive(Embed)]` with `#[json]`:

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

    #[json]
    preferences: UserPreferences,
}
```

`UserPreferences` is one `jsonb` column on PostgreSQL, one BSON sub-document
on MongoDB. Without `#[json]`, the same `UserPreferences` would expand to
`preferences_theme`, `preferences_notifications_email`, and so on — the
existing embed behavior.

### Collections at the root: `Vec<T>` and `Map<K, V>`

`Vec<T>`, `HashMap<String, T>`, and `BTreeMap<String, T>` on a model also
require `#[json]`:

```rust
#[derive(toasty::Model)]
struct User {
    #[key] #[auto]
    id: u64,

    #[json]
    tags: Vec<String>,

    #[json]
    metadata: HashMap<String, String>,
}
```

Without `#[json]`, the macro rejects these field types — there is no
implicit collection storage today. A future `#[column(type = array)]`
will add native PostgreSQL `text[]` arrays as a separate option.

The path API for collection fields mirrors `std`. `User::FIELDS.tags()`
is a typed path of element type `String` exposing:

- `.contains(value)` — the array contains this element. (`Vec::contains`.)
- `.is_superset(set)` — the array contains every element of `set`.
  (`HashSet::is_superset`.)
- `.intersects(set)` — the array shares at least one element with `set`.
  (Negation of `HashSet::is_disjoint`; no positive `std` form exists.)
- `.len()` — the array length as a typed numeric expression.
- `.is_empty()` — equivalent to `.len().eq(0)`.

`User::FIELDS.metadata()` is a typed path of key type `String` and value
type `String`:

- `.contains_key(key)` — the map has this key. (`HashMap::contains_key`.)
- `.keys()` — a set view exposing the same set methods as `Vec` paths
  (`.contains`, `.is_superset`, `.intersects`).
- `.values()` — an iterable view supporting `.any(|v| …)` and
  `.all(|v| …)` (Tier 2 below).
- `.len()`, `.is_empty()` — as above.

Accepted field types for `#[json]`:

- Any `#[derive(Embed)]` struct or enum.
- `Vec<T>`, `HashSet<T>`, `BTreeSet<T>` of a JSON-compatible `T`.
- `HashMap<String, T>`, `BTreeMap<String, T>`.
- `Option<T>` of any of the above.
- Bare scalars (`i64`, `String`, `bool`, etc.) — useful when the column
  must accept multiple JSON types.

### Storage modifiers

```rust
#[json]              // jsonb on PG, BSON sub-doc on Mongo, JSON on MySQL, JSON1 on SQLite
preferences: UserPreferences,

#[json(text)]        // PostgreSQL `json` (text). Other backends ignore the modifier.
audit_blob: AuditBlob,
```

`#[json]` is the right default in nearly every case. Reach for `#[json(text)]`
only when exact-byte preservation matters — audit trails, JSON received
from a third party that you re-emit verbatim.

### Tier 1: equality, range, and null on a nested field

Path access on `#[json]` fields uses the same accessor chain as
column-expanded embeds:

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
matches?" Pass a partial value of the embed's type:

```rust
User::all().filter(
    User::FIELDS.preferences().contains(UserPreferencesPartial {
        theme: Some("dark".into()),
        ..Default::default()
    })
);
```

Toasty generates a `{Type}Partial` companion struct for every `#[json]`
embed where every field is `Option`, mirroring the existing update-builder
pattern. `Default::default()` leaves a field unspecified; `Some(x)`
includes it in the predicate.

### Tier 1: collection predicates

The collection methods introduced above also work on `Vec` and `Map` paths
inside embeds:

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

### Tier 1: full-value replacement

```rust
user.update()
    .preferences(new_preferences)
    .exec(&mut db)
    .await?;
```

Whole-value writes work on every backend with no special operator support.

### Tier 1: nested patch via `stmt::patch`

`stmt::patch` extends to JSON paths. A patch updates one field inside the
document and leaves the rest of the document unchanged:

```rust
user.update()
    .preferences(stmt::patch(
        UserPreferences::FIELDS.theme(),
        "light",
    ))
    .exec(&mut db)
    .await?;

user.update()
    .preferences(stmt::patch(
        UserPreferences::FIELDS.notifications().push(),
        false,
    ))
    .exec(&mut db)
    .await?;
```

Patches compose: pass several to one `update()` call to apply them in a
single statement.

### Tier 1: numeric increment

```rust
user.update()
    .stats(stmt::increment(
        UserStats::FIELDS.login_count(),
        1,
    ))
    .exec(&mut db)
    .await?;
```

Atomic on backends that support it (PostgreSQL `jsonb_set` with a computed
expression, MongoDB `$inc`); falls back to read-modify-write on
backends without an atomic increment.

### Tier 1: array push and remove

```rust
user.update().tags(stmt::push("admin")).exec(&mut db).await?;

user.update().tags(stmt::push_all(["admin", "verified"])).exec(&mut db).await?;

user.update().tags(stmt::remove_eq("guest")).exec(&mut db).await?;
```

`push` appends; `remove_eq` removes every element equal to the value.

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
`EXISTS` over `jsonb_array_elements`; Mongo `$elemMatch`). `all` is
supported on Mongo natively and via `NOT any(NOT pred)` on PG.

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
    .tags(stmt::remove_at(2))           // remove array index 2
    .exec(&mut db)
    .await?;
```

`stmt::remove_at` matches `Vec::remove`. Out-of-bounds is a no-op rather
than an error, since per-row failure semantics on a bulk update are
rarely useful.

### Indexes

Index declarations live on the model alongside other indexes. Two forms:

```rust
#[index(json_gin(preferences))]                 // GIN over the whole jsonb column
#[index(json_path(preferences => theme))]       // expression index on a single path
```

`json_gin` produces a PG GIN index (`jsonb_ops` opclass), a MongoDB
wildcard index, or a no-op on backends without one. `json_path` produces
a PG B-tree expression index or a MongoDB path index. Index DDL is
declarative; the planner picks indexes based on the query shape.

## Behavior

**Storage.** A `#[json]` field maps to one column. The column's database
type is `jsonb` on PostgreSQL by default, `JSON` on MySQL, JSON1 (`TEXT`
with `CHECK json_valid`) on SQLite, and a BSON sub-document on MongoDB.
`#[json(text)]` overrides the PostgreSQL default to `json`.

**Encoding.** Toasty serializes the embed value to JSON using the same
representation it would use for column-expanded embeds, packed into one
document. Enum discriminators encode as a `__type` key by default;
`#[column(rename = ...)]` adjusts it. Numeric types preserve Rust width
where the backend supports it (Mongo Int32/Int64; PG `jsonb` numeric).
Floating-point NaN and infinity are rejected at encode time — JSON has
no representation for them.

**Column-rename attributes on JSON-stored embeds.** A `#[column("name")]`
annotation on a field of an embed type used as `#[json]` is an error at
schema build time. The annotation renames a SQL column suffix in the
column-expanded case; under `#[json]` there is no column to rename, and
JSON keys come from the Rust field name. Renaming JSON keys is a future
feature (likely `#[json(rename = "...")]`).

**Null vs missing key.** `Option<T>` writes nothing for `None` and a JSON
value for `Some`. Reading distinguishes:

- Absent key → `None`.
- Explicit JSON `null` → `None`, with `#[json(strict_nulls)]` opting into
  an error instead.

`is_none()` matches both. `is_absent()` and `is_null()` match only one
each.

**Patch semantics.** A `stmt::patch(path, value)` writes `value` at `path`,
creating intermediate objects as needed. A patch that walks through a
field whose current type is incompatible (e.g. patching `notifications.email`
when `notifications` is currently a JSON array) returns a runtime error
on the affected row.

**Array writes.** `stmt::push` appends to the array, creating it if
absent. `stmt::remove_eq` removes every matching element. `stmt::remove_at`
removes a single index; out-of-bounds is a no-op.

**Concurrent updates.** PostgreSQL `jsonb_set` rewrites the entire
document; concurrent patches to disjoint paths on the same row are not
independent — the last write wins for the whole column. MongoDB `$set`
on disjoint paths is independent. Code that depends on per-path atomicity
should not rely on it on PostgreSQL.

## Backend mapping

| Operation | PostgreSQL `jsonb` | MongoDB | SQLite (JSON1) | MySQL |
|---|---|---|---|---|
| Path equality | `col->'a'->>'b' = ...` | `{"a.b": ...}` | `json_extract` | `JSON_EXTRACT` |
| Containment | `@>` | structural match | `json_each` + filter | `JSON_CONTAINS` |
| `contains_key` | `?` | `$exists` | `json_extract IS NOT NULL` | `JSON_CONTAINS_PATH` |
| `contains` (array) | `@>` | `{arr: v}` | `json_each` | `JSON_CONTAINS` |
| `is_superset` | `@>` | `$all` | `json_each` | `JSON_CONTAINS` |
| `intersects` | `?\|` | `$in` (per-element) | `json_each` | `JSON_OVERLAPS` |
| `len` | `jsonb_array_length` | `$size` | `json_array_length` | `JSON_LENGTH` |
| `any` predicate | `EXISTS` over `jsonb_array_elements` | `$elemMatch` | `EXISTS` over `json_each` | `JSON_TABLE` |
| Patch one path | `jsonb_set` | `$set` | rewrite via `json_set` | `JSON_SET` |
| Increment | `jsonb_set` with cast | `$inc` | `json_set` arith | `JSON_SET` arith |
| `push` | `\|\|` array concat | `$push` | `json_insert` | `JSON_ARRAY_APPEND` |
| `remove_eq` | `jsonb_set` minus filter | `$pull` | rewrite | rewrite |
| `remove_at` | `jsonb_path` minus | `$unset` + `$pull` | rewrite | `JSON_REMOVE` |
| `unset` (key) | `-` | `$unset` | `json_remove` | `JSON_REMOVE` |

### Future MongoDB gaps

Items the API expresses cleanly but the eventual Mongo driver will need to
work through:

- **BSON-only types in `#[json]` embeds.** `ObjectId`, `Date`, `Decimal128`,
  and `UUID` have no JSON representation. A Mongo-backed model must be
  able to declare these types in an embed (e.g. `created_at:
  bson::DateTime`) and have them encoded as BSON natively. Until then,
  `#[json]` embeds containing such types are rejected at schema build
  time.
- **Positional array operators.** Mongo's `$[<id>]` and `arrayFilters`
  let `$set` target specific elements within nested arrays atomically.
  Toasty's `stmt::patch` over an array path needs to compile to those
  operators on Mongo to retain atomicity; on PG it falls back to whole-
  document rewrite anyway.
- **Wildcard / multikey index DDL.** `json_gin` and `json_path` cover
  the common cases, but Mongo's compound multikey rules and wildcard
  projection do not have a one-line DDL today. A `#[index(json(...))]`
  form with finer knobs is likely needed.
- **Sharding by JSON-path key.** Mongo shard keys can be JSON paths.
  Toasty's key model is single-field; this is out of scope for v1 and
  may need a broader composite-key story.
- **Map keys containing `.`.** Mongo path notation uses `.` as a
  separator; map keys containing literal dots need escaping or
  rejection. Decided per the open question below.

## Edge cases

- **Mixed-type values at a path.** Filtering `User::FIELDS.metadata().get("count").eq(5)`
  matches when the value at `count` is the JSON number 5; it does not
  coerce the JSON string `"5"`. Strict typing is the default.
- **Empty document vs missing column.** `NOT NULL` `#[json]` fields
  default to `{}` (objects), `[]` (arrays), or the embed's default;
  `Option<T>` allows SQL `NULL`.
- **Document size limits.** PostgreSQL TOAST caps individual values near
  1 GB; MongoDB caps documents at 16 MB. Toasty does not enforce a
  smaller limit. Inserts exceeding the backend limit surface a driver
  error.
- **Floating-point edge values.** NaN and infinity are rejected at encode
  time. Negative zero round-trips as zero on PostgreSQL `jsonb` (it
  stores `numeric`).
- **Reading legacy data.** A row whose JSON does not match the current
  schema (extra keys, missing keys, wrong type at a path) deserializes
  field-by-field; missing required fields surface as decode errors,
  extra keys are dropped silently. A future `#[json(strict)]` modifier
  could opt into rejecting extra keys.
- **Schema drift between writes.** A patch onto a row whose document
  was written by an older or external writer may produce surprising
  shapes (e.g. a key that was a string is now an object). Toasty does
  not validate the existing document before patching.

## Driver integration

**New schema artifact.** Drivers see a new `ColumnType::Json { binary:
bool }`. SQL drivers map `binary: true` to their dialect's JSONB type
(`jsonb` on PG; `JSON` on MySQL; `TEXT CHECK json_valid(...)` on SQLite,
since SQLite has no separate type). `binary: false` maps to the dialect's
text JSON (`json` on PG; ignored elsewhere).

**New operations.** SQL drivers gain new statement nodes that the SQL
serializer renders to dialect-specific operators:

- `stmt::Expr::JsonPath { value, path }` — path traversal for reads.
- `stmt::Expr::JsonContains { lhs, rhs }`, `JsonContainsKey`,
  `JsonIsSuperset`, `JsonIntersects`.
- `stmt::Expr::JsonArrayLength`, `JsonArrayAny { var, body }`,
  `JsonArrayAll { var, body }`.
- Update RHS forms: `stmt::Assign::JsonSet`, `JsonInc`, `JsonPush`,
  `JsonRemoveEq`, `JsonRemoveAt`, `JsonUnset`.

Each is gated behind a capability flag (`Capability::JsonContains`, etc.).
The planner reads capabilities to decide whether to push the operator
to the driver or fall back to an in-memory implementation. Drivers that
implement none of the JSON capabilities still work — every JSON-aware
predicate compiles to load-and-filter, every JSON-aware update compiles
to read-modify-write — they just lose the per-operator optimizations.

**MongoDB driver (future).** The driver compiles statement nodes directly
to its query and update document forms; SQL serialization does not apply.
The `Capability::JsonInPlaceAtomic` flag exposes whether disjoint-path
patches are independent. Toasty's planner can use this for transaction
ordering decisions.

**Out-of-tree drivers.** Existing drivers compile unchanged. New
operations are gated behind capability flags; absent flags fall back to
the load-and-rewrite paths, which are wholly server-driven through the
existing `QuerySql` and `Insert` operations.

## Alternatives considered

**Keep `#[serialize(json)]` and add no new attribute.** Reuse the existing
opaque-blob attribute for the queryable case as well, distinguishing by
type (Embed vs serde). Rejected because the storage and query
capabilities differ enough that two attributes are clearer than one
overloaded one. `#[serialize(json)]` remains for cases where the user
wants a serde-only escape hatch with no querying.

**Implicit JSON storage for `Vec<T>` and `Map<K, V>`.** Skip the `#[json]`
on collection fields since they have no column-expanded representation
anyway. Rejected: explicit storage choice means the user can later opt
into a different representation (PG-native `text[]`, sidecar table) by
swapping the attribute, with no silent storage change. Errors point at
the missing attribute with a clear message.

**Always store embeds as JSON; no flag.** Removes the choice. Rejected:
column-expanded embeds give per-field indexes, smaller rows, and existing
SQL-tuning techniques that JSON storage forecloses. The choice is
load-bearing.

**Two attributes, `#[json]` and `#[jsonb]`.** Surfaces PG-specific naming.
Rejected: most backends have one JSON type. `#[json(text)]` puts the rare
modifier on the rare path.

**Document-collection API distinct from Embed.** A separate, Mongo-flavored
"collection of documents" surface alongside the relational one. Rejected:
two parallel modeling APIs is more surface than the value warrants when
embed already covers nested data.

**Naming after Mongo (`has`, `has_all`, `has_any`).** Rejected for Rust
idiom: `Vec::contains`, `HashSet::is_superset`, and the `intersects`
form (negation of `is_disjoint`) read more naturally to Rust users.

## Open questions

- **Default `create_if_missing` for `stmt::patch`.** PostgreSQL's
  `jsonb_set` takes a flag; Mongo's `$set` always creates. True is more
  forgiving; false catches typos. Blocking acceptance.
- **Discriminator key name for enums.** `__type`? `$type`? Configurable
  per enum? Blocking implementation.
- **Map keys containing `.`.** Mongo path notation uses `.` as a key
  separator; allowing arbitrary string keys requires escaping on encode
  or rejection. Blocking implementation for the Mongo driver;
  deferrable for v1 PG-only.
- **`HashMap` ordering.** PG `jsonb` sorts keys; SQLite preserves input
  order; Mongo preserves input order. Document the lack of ordering
  guarantee or normalize on encode? Deferrable.
- **Index DDL syntax.** The `json_gin` / `json_path` forms above are a
  starting point; they may want subkey selection, opclass selection on
  PG, and partial-index conditions. Deferrable.
- **Renaming JSON keys.** `#[json(rename = "...")]` on an embed field
  is the natural form. Decide before or alongside implementation.
  Deferrable.

## Out of scope

- **Raw JSON path expressions.** A `path_match("$.a[*] ? (@.b > 1)")`
  escape hatch for queries the typed accessors cannot express. Defer
  until the typed surface proves insufficient.
- **DynamoDB JSON support.** DynamoDB has its own document model
  (Map / List attributes) with different operators and indexing rules.
  A separate design will cover how `#[json]` fields encode there.
- **Native PostgreSQL arrays.** `#[column(type = array)]` on a `Vec<T>`
  to opt into `text[]` instead of `jsonb`. A separate, smaller design.
- **Schema migrations for nested document shape changes.** Migrating a
  field from string to object across all rows is a bulk read-modify-
  write; no special migration primitives in this design.
- **Full-text search over JSON.** Tracked as a separate roadmap item.
- **Server-side aggregation pipelines.** Mongo's `$group` /
  `$lookup` and PG's `jsonb_agg` are aggregation features; covered by
  the broader aggregation design.
- **JSON Schema validation.** Per-field structural validation is a
  separate feature; check constraints already exist as a roadmap item.
