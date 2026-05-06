# Deferred Fields

## Summary

A `#[deferred]` field attribute paired with a `Deferred<T>` field
wrapper marks a column as not loaded by default. Records returned from
a query carry the field in an unloaded state; accessing the value
requires either a follow-up `.exec()` call or preloading through
`.include()`. A `Deferred<T>` field whose inner type is not `Option<_>`
is a required argument at create time, exactly like any other
non-nullable field. The API mirrors `BelongsTo<T>`: load on demand with
`.exec()`, preload with `.include()`, read a preloaded value with
`.get()`. One feature covers SQL and DynamoDB.

## Motivation

Some columns are large, expensive to fetch, or rarely used. A `Document`
that carries a multi-megabyte `body`, an `Image` with raw bytes, an
audit `Event` with a JSON payload — most code paths list, filter, or
display these records without ever touching the heavy column. Today
every query pulls every column. Users who care work around it by
splitting the heavy column into a sibling table and joining when they
need it, which doubles the schema, complicates writes, and leaks the
workaround into every query.

The pattern is well-established elsewhere — Hibernate's `FetchType.LAZY`,
ActiveRecord's `select`, Diesel's `select(columns)`. Toasty has no
equivalent. Filling the gap with a typed wrapper keeps the cost visible
in the model definition and reuses Toasty's existing
`include`/relation-loader machinery instead of inventing a per-call
projection mechanism.

## User-facing API

### Marking a field deferred

Tag the field with `#[deferred]` and wrap its type in `Deferred<T>`:

```rust
#[derive(Debug, toasty::Model)]
struct Document {
    #[key]
    #[auto]
    id: u64,

    title: String,

    #[deferred]
    body: toasty::Deferred<String>,

    #[deferred]
    summary: toasty::Deferred<Option<String>>,
}
```

The attribute is what tells the macro to treat the field as deferred —
to skip it from the default projection, generate a load method, and
register a non-`Primitive` schema entry. The wrapper type carries the
unloaded-state runtime API. Both are required; one without the other
is a compile error. This matches Toasty's existing pattern for
relations, where `#[belongs_to]` and `BelongsTo<T>` always travel
together.

`body` is required — every `Document` row stores a value. `summary` is
nullable. Both are excluded from the default `SELECT` list. Querying for
`Document` returns records with `title` populated and the deferred
fields in an unloaded state.

`#[deferred]` works with primitives or embedded types (`#[derive(Embed)]`
structs and enums) at the root model, and inside embedded types
themselves. It does not compose with `BelongsTo`, `HasMany`, or
`HasOne` — relations are already lazy.

A deferred embedded value omits all of the embed's columns from the
default projection:

```rust
#[derive(toasty::Embed)]
struct Metadata {
    author: String,
    notes: String,
}

#[derive(toasty::Model)]
struct Document {
    #[key] #[auto] id: u64,
    title: String,

    #[deferred]
    metadata: toasty::Deferred<Metadata>,
}
```

A deferred field inside an embedded type defers just that column even
when the embed itself is eager:

```rust
#[derive(toasty::Embed)]
struct Metadata {
    author: String,

    #[deferred]
    notes: toasty::Deferred<String>,
}
```

In the second shape, a default load of `Document` populates
`title` and `metadata.author`; `metadata.notes` is unloaded.

### Default queries

A normal query loads the eager fields and leaves the deferred ones
unloaded:

```rust
let doc = Document::filter_by_id(id).get(&mut db).await?;

println!("{}", doc.title);          // loaded
assert!(doc.body.is_unloaded());    // deferred — no value yet
```

`doc.body.get()` panics in this state. The contract matches `BelongsTo`:
`.get()` is the synchronous accessor for a value the engine has already
materialized.

### Loading on demand

Call the field method to fetch the value:

```rust
let body: String = doc.body().exec(&mut db).await?;
```

For `Deferred<Option<T>>` the method returns `Option<T>`. The call
issues a single-row read keyed on the record's primary key —
`SELECT body FROM documents WHERE id = ?` for SQL,
`GetItem` with a `ProjectionExpression` for DynamoDB.

`.exec()` does **not** populate the field on the record. Mutating the
in-memory record from a `&self` accessor would be surprising and forces
interior mutability. Re-issuing the same load returns the value again.
Code that expects to read the value many times should reach for
`.include()` instead.

### Preloading with `.include()`

`.include()` extends the original query's projection so deferred fields
arrive with the rest of the record:

```rust
let doc = Document::filter_by_id(id)
    .include(Document::fields().body())
    .get(&mut db)
    .await?;

let body: &String = doc.body.get();   // synchronous, no query
```

Multiple `.include()` calls coalesce into a single projection. Mixing
deferred fields and relation includes works as expected:

```rust
let doc = Document::filter_by_id(id)
    .include(Document::fields().body())
    .include(Document::fields().summary())
    .include(Document::fields().author())   // BelongsTo
    .get(&mut db)
    .await?;
```

Across a result set `.include()` is the way to avoid N+1: every record
in the batch gets its deferred fields populated by the same query.

`.include()` paths traverse embedded types using the same field-path
chain that filters use today. Given the embed-with-deferred-field
shape from above:

```rust
let doc = Document::filter_by_id(id)
    .include(Document::fields().metadata().notes())
    .get(&mut db)
    .await?;
```

Including a deferred embedded value loads the embed's eager columns
in one shot but leaves any sub-fields that are themselves deferred in
the unloaded state — included paths add what they name, no more. To
load both the embed and a deferred sub-field of it, include both
paths:

```rust
let doc = Document::filter_by_id(id)
    .include(Document::fields().metadata())          // embed itself
    .include(Document::fields().metadata().notes())  // its deferred sub-field
    .get(&mut db)
    .await?;
```

### Creating

A required `Deferred<T>` is a required argument to `create!`, same as
any other non-nullable field:

```rust
let doc = toasty::create!(Document {
    title: "Hello",
    body: "...the long body...",
})
.exec(&mut db)
.await?;

assert_eq!(doc.title, "Hello");
assert!(doc.body.is_unloaded());     // not echoed back by default
```

The returned record has `title` populated (it was already on the wire)
and `body` unloaded. The create round-trip does not echo deferred
columns back; loading the value requires a follow-up `doc.body().exec(...)`
or a separate query with `.include(...)`. Whether `create!` should
gain an opt-in for echoing deferred fields back from `RETURNING` is
tracked under open questions.

A `Deferred<Option<T>>` is optional; omitting it stores `NULL`.

The static-create-field check ([static-assertions-create-macro.md])
treats `Deferred<T>` like any other non-nullable field — `create!` fails
to compile if a required deferred field is missing.

[static-assertions-create-macro.md]: ./static-assertions-create-macro.md

### Updating

`Deferred<T>` does not change the update API. Setters appear on the
update builder regardless of whether the field is loaded:

```rust
doc.update().body("new body").exec(&mut db).await?;
```

The update does not require a prior `.exec()` of the deferred field. A
loaded value on the in-memory record is left as-is by the update — the
caller is responsible for refreshing if they want to read the new value
without an extra round-trip.

### Filtering and sorting

A deferred field appears in `Model::fields()` and can be used in any
predicate or sort expression. Using a deferred field in a filter does
**not** load it into the result:

```rust
let docs = Document::filter(Document::fields().body().contains("rust"))
    .exec(&mut db)
    .await?;

// `body` is in the WHERE clause but not the SELECT list.
assert!(docs[0].body.is_unloaded());
```

If the caller wants the matched bodies, chain `.include(...)`.

## Behavior

**Default projection.** Toasty's compiled query strips deferred fields
from the column list it requests from the driver. The list of deferred
columns is part of the model schema and is consulted at lower-time when
the engine expands `*` into concrete columns.

**`Deferred<T>` value lifecycle.** A `Deferred<T>` carries `Option<T>`
internally. It is `None` after a default load, `Some(...)` after a
preload, and `None` again after `.unload()`. `is_unloaded()` and
`.get()` mirror `BelongsTo`. `.unload()` returns the field to the
unloaded state without touching the database.

**`.exec()` on a deferred field.** Issues a single-record read keyed on
the model's primary key. The result type is `T` (so `String` for
`Deferred<String>` and `Option<U>` for `Deferred<Option<U>>`). The
in-memory record is not mutated; the value is returned by ownership.

**`.include()` on a deferred field.** Extends the parent query's column
list with the deferred column. No extra round-trip — the deferred
column rides on the existing query.

**Required at create.** Whether a `Deferred<T>` field is required is
decided by `T`: a non-`Option` `T` is required, an `Option<T>` is not.
The compile-time check in `create!` keys on the same `Field::NULLABLE`
trait constant the rest of the macro uses.

**Update echo.** An update does not change loaded-ness. If the field
was loaded before the update, the in-memory copy is stale until the
caller re-reads. We do not propagate the new value into the record
because update builders today take expressions, not always values
(`set = current + 1`); the engine cannot in general compute the
post-update scalar without a follow-up read.

**Interactions.**

- *Embedded types.* `Deferred<T>` where `T` is an embedded struct or
  enum defers every column the embed contributes. `#[deferred]` on a
  field inside an embed defers just that column wherever the embed is
  used. The two are independent: an eager embed may contain deferred
  fields, and a deferred embed may contain eager fields (which are
  still pulled when the embed itself is included).
- *Relations.* `Deferred<BelongsTo<T>>` is a build error. Use `.include()`
  to control relation loading.
- *Indexes.* A `#[index]` on a deferred field is allowed and creates an
  index on the underlying column. Filtering still works without
  loading.
- *`#[version]`.* `#[version]` on a `#[deferred]` field is a build
  error. A version counter that is absent from the default load
  cannot drive instance-update conflict detection.
- *`.include()` ordering.* `.include()` is order-independent;
  duplicates are deduped.

## Edge cases

**Forgetting to `.include()` in a hot loop.** A `for` loop over a
`Vec<Document>` that calls `doc.body().exec(...)` is N+1 by definition.
Toasty does not detect this; the user-facing fix is `.include()`. The
`.exec()` form exists exactly for the cases where a single record needs
the value.

**Update without a load.** `doc.update().body("...").exec(...)` works
even if `body` was never loaded. The update statement is a write —
nothing about it requires the prior value.

**Stream results.** A streamed query (when streaming lands) yields
records with deferred fields unloaded unless the stream itself was set
up with `.include()`. Callers must not call `.exec()` on a deferred
field per stream item.

**Removing a row between load and `.exec()`.** A `Deferred<T>::exec()`
that finds the row gone returns `Error::record_not_found`. The error
type matches the rest of the engine's missing-row vocabulary.

**Unique constraints.** Deferred is a load-time concern; constraints
are write-time. A `#[unique]` deferred column behaves identically to a
non-deferred one for inserts and updates.

## Driver integration

The feature is a schema annotation plus a projection-list change. Most
of the work lives in lower/plan; drivers see the same operations they
already handle but with shorter projection lists.

### Schema

- `app::Field` carries a `deferred: bool` flag set by the macro when the
  field type is `Deferred<T>`.
- The schema build step asserts `deferred` cannot coexist with relation
  field types or with `#[derive(Embed)]` types.

### Engine

- Lowering expands the model's column projection by walking `Fields`
  and skipping any field with `deferred = true`, unless the lowered
  query carries an `include` of that field.
- The `include` lowering already supports relation projections; it gains
  a "scalar deferred field" arm that simply re-adds the column.
- The compiled `stmt::Query`, `stmt::Update`, and the conditional-update
  plans are unchanged in shape — only the column list inside the
  existing `Returning` differs.

### SQL drivers

Nothing new. The SQL serializer already accepts a column list; the
shorter list flows through `SELECT col1, col2 FROM …`. The on-demand
load is `SELECT <column> FROM <table> WHERE <pk> = ?`, also already
supported. SQL drivers do not need a capability flag for this feature.

### DynamoDB

The DynamoDB driver translates the engine's column list into a
`ProjectionExpression`. With `Deferred<T>`, the default projection
omits the deferred attributes; an `.include()` adds them back. On-demand
load is `GetItem` with a `ProjectionExpression` listing only the
deferred attribute and reusing the record's already-known PK. No new
`Operation` variant.

### Backward compatibility for out-of-tree drivers

Drivers do not see a new `Operation` and do not need a new capability
flag. A driver that ignored projection lists in the past (i.e. always
returned every attribute) keeps working — the engine discards the
extra columns at decode time, and the user-visible behavior is "the
deferred field is loaded even though it shouldn't be." That is the
worst-case compat outcome and is no more wrong than the pre-feature
status quo.

## Alternatives considered

**`Deferred<T>` type alone, no attribute.** Detect deferred fields by
syntactically matching the wrapper type, the way the macro detects
`Option<T>` and `Vec<T>`. Rejected because the macro does not actually
discriminate fields by their Rust type — it discriminates by attribute,
producing a `FieldTy` variant per field at parse time. Today every
non-`Primitive` `FieldTy` (`BelongsTo`, `HasMany`, `HasOne`) requires
an attribute. The deferred flavor needs to dispatch through the same
seams: a new `FieldTy::Deferred` variant, an arm in
`expand_model_relation_methods` to emit the per-field load method,
arms in the field-path and schema-emission match sites. Driving that
machinery off attribute presence is consistent with how the rest of
the macro works. The `Option<T>` / `Vec<T>` analogy does not apply —
those are matched through `<T as Field>::NULLABLE`-style trait
dispatch on a primitive field, not by pattern-matching the type name.
A side benefit: `#[deferred]` survives type aliases (`type Body =
Deferred<String>;`) and other elaborations the syntactic matcher would
miss.

**Attribute alone, no wrapper type.** `#[deferred] body: String` with no
wrapper. Rejected because `String` has no "not loaded" state — the
field would either need to default to `String::new()` (silently lying)
or panic on access (no observable type-level signal). The wrapper makes
the unloaded state explicit and gives the load API a place to live.

**Per-query opt-in via projection.** Instead of marking fields in the
schema, let callers write `Document::all().select(...)` or
`.exclude(Document::fields().body())`. Rejected because the heavy
column is a property of the schema, not the call site — every caller
that doesn't think about it pays the cost. A schema-level default
flips the polarity to "explicit when needed." Per-query projection is a
separate, future feature for ad-hoc shaping.

**Sibling-table workaround.** Move the heavy column into its own
one-to-one table and use `BelongsTo` to load it. Rejected as a
first-class feature because it doubles the schema, complicates writes
(two inserts per logical row), and prevents transactional updates of
the row plus the heavy column without the user wiring up a
transaction.

**Eager population on `.exec()`.** Have `doc.body().exec(...)` mutate
the record (set `body = Some(value)`) instead of returning by value.
Rejected because it requires `&mut self` (or interior mutability) on
every load call site, and silently changing in-memory state from a
load looks too much like preloading. Returning by value keeps the
direction of data flow explicit; callers who want to cache write back
themselves.

## Open questions

- **Loaders on a `Vec<Model>`.** `.include()` covers the batch case at
  query time, but a user who already has a `Vec<Document>` from a prior
  query has no batch loader for a deferred field. A
  `Document::load_body(&mut docs, &mut db)` helper or a method on the
  collection is plausible. *Deferrable.*
- **Returning the deferred value from `create`/`update`.** `RETURNING`
  on PostgreSQL and `ReturnValues = ALL_NEW` on DynamoDB can echo the
  written value back without a follow-up read. Should `create!` and
  `update().exec()` opportunistically populate `Deferred<T>` from
  `RETURNING`? Probably yes, controlled by a per-call `.include(...)`
  on the write builder so the caller pays only when they want the value
  back. *Deferrable.*
- **`Stream` integration.** When streaming results lands ([#324]), the
  contract for deferred fields under streaming should match the
  collection-query contract (unloaded unless preloaded). *Blocking on
  the streaming design, not this one.*

[#324]: https://github.com/tokio-rs/toasty/issues/324

## Out of scope

- **Per-call column projection** — `.select(...)` / `.exclude(...)` on a
  query. A separate feature; deferred is the schema-default version.
- **Deferred relations** — `BelongsTo`, `HasMany`, and `HasOne` are
  already lazy; `Deferred<Relation<T>>` does not add anything.
- **Cross-row batching of `.exec()`** — a Vec-level batch loader is
  noted in open questions; not part of this design.
- **Compression or out-of-band storage** — moving the column into S3 or
  similar is an application concern, unrelated to load timing.
