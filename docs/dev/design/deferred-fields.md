# Deferred Fields

## Summary

A `Deferred<T>` field is not loaded by default. Records returned from a
default query carry the field in an unloaded state. Callers use
`.include()` to load the value with the parent query and `.get()` to read
an already-loaded value.

The wrapper type is the API marker. There is no field attribute for
deferral. The macro records deferral by reading the `Field::DEFERRED`
associated constant for the declared field type.

`Deferred<T>` where `T` is not `Option<_>` is required at create time,
like any other non-nullable field. `Deferred<Option<T>>` is optional.

## Motivation

Some columns are large, expensive to fetch, or rarely used. A `Document`
body, raw image bytes, or an audit-event JSON payload may be needed in
only a few code paths. Loading those columns for every list query wastes
bandwidth and decode work.

`Deferred<T>` makes the load behavior visible in the model definition
and keeps normal queries cheap. The value still belongs to the same
model; users do not need to split a large column into a sibling table to
avoid default loads.

## User-Facing API

### Marking a Field Deferred

Wrap the field type in `Deferred<T>`:

```rust
#[derive(Debug, toasty::Model)]
struct Document {
    #[key]
    #[auto]
    id: u64,

    title: String,
    body: toasty::Deferred<String>,
    summary: toasty::Deferred<Option<String>>,
}
```

`body` is required and `summary` is nullable. Both fields are omitted
from the default projection:

```rust
let doc = Document::filter_by_id(id).get(&mut db).await?;

assert_eq!("Hello", doc.title);
assert!(doc.body.is_unloaded());
```

Calling `.get()` on an unloaded field panics. `.get()` is synchronous
because it only reads a value already present on the record.

### Loading With `.include()`

Use `.include()` to load a deferred field with the parent query:

```rust
let doc = Document::filter_by_id(id)
    .include(Document::fields().body())
    .get(&mut db)
    .await?;

let body: &String = doc.body.get();
```

`.include()` adds the deferred column to the existing query. Multiple
includes coalesce and can be mixed with relation includes:

```rust
let doc = Document::filter_by_id(id)
    .include(Document::fields().body())
    .include(Document::fields().summary())
    .include(Document::fields().author())
    .get(&mut db)
    .await?;
```

Across a result set, a single `.include()` loads the deferred field for
every returned record.

### Embedded Types

`Deferred<T>` is supported for primitive fields and embedded types
(`#[derive(Embed)]` structs and enums):

```rust
#[derive(Debug, toasty::Embed)]
struct Metadata {
    author: String,
    notes: String,
}

#[derive(Debug, toasty::Model)]
struct Document {
    #[key]
    #[auto]
    id: u64,

    title: String,
    metadata: toasty::Deferred<Metadata>,
}
```

A deferred embedded value omits every column the embedded type
contributes. Including the embedded field loads the embedded value.

`Deferred<T>` is also valid inside an embedded type:

```rust
#[derive(Debug, toasty::Embed)]
struct Metadata {
    author: String,
    notes: toasty::Deferred<String>,
}
```

To load that sub-field, include its field path:

```rust
let doc = Document::filter_by_id(id)
    .include(Document::fields().metadata().notes())
    .get(&mut db)
    .await?;
```

When constructing an embedded value directly, a deferred sub-field
accepts the inner value through `From<T>`:

```rust
Metadata {
    author: "Alice".to_string(),
    notes: "the note".to_string().into(),
}
```

### Creating

A required `Deferred<T>` is a required argument to `create!`:

```rust
let doc = toasty::create!(Document {
    title: "Hello",
    body: "...the long body...",
})
.exec(&mut db)
.await?;

assert_eq!("...the long body...", doc.body.get());
```

The record returned by `create!` carries the value the caller just
provided. A later query against the same row returns the field unloaded
unless that query includes it.

Omitting a `Deferred<Option<T>>` field stores `NULL`.

### Updating

Updating a deferred field does not require the field to be loaded:

```rust
doc.update().body("new body").exec(&mut db).await?;

assert_eq!("new body", doc.body.get());
```

The caller supplies the new value, so Toasty stores that value on the
in-memory record after the update completes.

### Filtering And Sorting

Deferred fields appear in `Model::fields()` and can be used in filters
and sort expressions. Referencing the column in `WHERE` or `ORDER BY`
does not load it into the result:

```rust
let docs = Document::all()
    .filter(Document::fields().body().like("%rust%"))
    .exec(&mut db)
    .await?;

assert!(docs[0].body.is_unloaded());
```

Add `.include(Document::fields().body())` when the result should carry
the body value.

## Behavior

**Default projection.** Lowering omits deferred fields from the model
projection unless the query includes them.

**Loaded state.** `Deferred<T>` stores an optional loaded value.
`is_unloaded()` reports whether the value is absent, `.get()` reads a
loaded value, and `.unload()` clears it without touching the database.

**Create and update values.** `create!` and update setters receive the
field value from the caller. The returned or mutated record keeps that
value loaded.

**Required fields.** Requiredness comes from the inner type. The
`create!` macro uses `Field::NULLABLE`: `Deferred<T>` is required and
`Deferred<Option<T>>` is optional.

**Relations.** Relation fields use `Deferred<_>` with relation
attributes such as `#[belongs_to]`, `#[has_many]`, or `#[has_one]`.
Scalar and embedded deferred fields do not use a deferral attribute.

**Indexes.** An index on a deferred field indexes the underlying column.
Filtering can use the index without loading the field.

**Version fields.** A deferred field cannot be a version field. The
version value must load with the record so instance updates can perform
conflict checks.

## Driver Integration

Drivers see the same operations they already handle with shorter
projection lists.

SQL drivers receive a `SELECT` list that omits deferred columns unless
the query includes them. DynamoDB receives a `ProjectionExpression` that
omits deferred attributes unless the query includes them. No driver
capability flag is required.

The app schema stores `deferred: bool` on each app field. Macro
expansion sets that flag from `<FieldType as Field>::DEFERRED`.

## Out Of Scope

- Per-field follow-up loaders. A deferred value is loaded through
  `.include()` on a query.
- Per-call column projection with `.select(...)`. That is a separate
  query-result feature.
- Cross-row batch loaders for records that were already loaded without
  the deferred field.
- Moving large values into external storage.
