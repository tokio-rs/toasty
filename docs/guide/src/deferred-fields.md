# Deferred Fields

A deferred field is a column Toasty omits from the default `SELECT`
list. Records returned from a query have the field unloaded; loading
the value requires a preload with `.include()`.

The pattern fits columns that are large, expensive to fetch, or rarely
read: a `Document` body, a binary blob, an audit-event JSON payload.
Without `Deferred<T>`, every list query reads every column whether the
caller needs it or not.

The API mirrors deferred relation fields: a synchronous `.get()` reads an
already-loaded value, and `.include()` preloads the value as part of
the parent query.

## Marking a field as deferred

Wrap the field type in `Deferred<T>`:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct Document {
    #[key]
    #[auto]
    id: u64,

    title: String,
    body: toasty::Deferred<String>,
}
```

The wrapper tells Toasty to omit the field from the default projection
and provides the unloaded-state runtime API.

A record from an ordinary query has `body` unloaded:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     body: toasty::Deferred<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let created = toasty::create!(Document {
#     title: "Hello",
#     body: "the long body",
# }).exec(&mut db).await?;
let doc = Document::filter_by_id(created.id).get(&mut db).await?;
assert!(doc.body.is_unloaded());
# Ok(())
# }
```

Use `.include()` so the value arrives on the record the query returns:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     body: toasty::Deferred<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let created = toasty::create!(Document {
#     title: "Hello",
#     body: "the long body",
# }).exec(&mut db).await?;
let doc = Document::filter_by_id(created.id)
    .include(Document::fields().body())
    .get(&mut db)
    .await?;

let body: &String = doc.body.get();   // synchronous, no query
# Ok(())
# }
```

`Deferred<T>` is supported on primitive fields and on embedded types
(`#[derive(Embed)]` structs and enums). It does not compose with
`#[belongs_to]`, `#[has_many]`, or `#[has_one]`. Relation fields use
`Deferred<_>` in the field type itself when they should be lazy.

A deferred embed value omits all of the embed's columns from the default
projection. Loading is the same as for a primitive: chain `.include()`
to preload alongside the parent query.

```rust
# use toasty::Model;
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

`Deferred<T>` is also valid on a primitive field *inside* an embedded
struct. It defers just that column wherever the embed is used; the
embed's other fields still load with the parent query.

```rust
# use toasty::Model;
#[derive(Debug, toasty::Embed)]
struct Metadata {
    author: String,
    notes: toasty::Deferred<String>,
}
```

To load such a sub-field on a parent query, name it in `.include()`:

```rust,ignore
let doc = Document::filter_by_id(id)
    .include(Document::fields().metadata().notes())
    .get(&mut db)
    .await?;
```

When the user constructs an embed value directly (struct-literal
syntax), a deferred sub-field accepts the inner value via `.into()`:

```rust,ignore
Metadata {
    author: "Alice".to_string(),
    notes: "the note".to_string().into(),
}
```

## Loaded state on create vs query

The record returned by `create!` is loaded with the deferred value the
caller just wrote — `.get()` reads it without a round-trip. A
subsequent query against the same row returns a separate record with
the deferred field unloaded:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     body: toasty::Deferred<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let created = toasty::create!(Document {
    title: "Hello",
    body: "the long body",
})
.exec(&mut db)
.await?;

// Loaded — the value the caller passed in.
assert_eq!("the long body", created.body.get());

// A separate query returns the deferred field unloaded.
let doc = Document::filter_by_id(created.id).get(&mut db).await?;
assert_eq!("Hello", doc.title);
assert!(doc.body.is_unloaded());
# Ok(())
# }
```

Calling `doc.body.get()` in the unloaded state panics. `.get()` is the
synchronous accessor for a value already loaded into the record; on an
unloaded field there is nothing to return.

## Preloading with `.include()`

`.include()` extends the parent query's projection so deferred fields
are loaded onto the same record returned by the query:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     body: toasty::Deferred<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let created = toasty::create!(Document {
#     title: "Hello",
#     body: "the long body",
# }).exec(&mut db).await?;
let doc = Document::filter_by_id(created.id)
    .include(Document::fields().body())
    .get(&mut db)
    .await?;

let body: &String = doc.body.get();   // synchronous, no query
# Ok(())
# }
```

The `.include()` call adds the deferred column to the existing query —
no extra round-trip. Multiple `.include()` calls on the same query
coalesce, and they combine with relation `.include()`s:

```rust,ignore
let doc = Document::filter_by_id(id)
    .include(Document::fields().body())
    .include(Document::fields().summary())
    .include(Document::fields().author())   // relation
    .get(&mut db)
    .await?;
```

Across a result set, `.include()` is the way to avoid N+1: a single
query loads the deferred fields for every record it returns.

## Filtering and sorting

Filtering or sorting on a deferred field references the column in
`WHERE` or `ORDER BY` without loading the value — only `.include()`
adds the field to the `SELECT` list:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     body: toasty::Deferred<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let alpha = toasty::create!(Document {
#     title: "First",
#     body: "alpha body",
# }).exec(&mut db).await?;
let docs = Document::filter_by_id(alpha.id)
    .filter(Document::fields().body().eq("alpha body".to_string()))
    .exec(&mut db)
    .await?;

assert_eq!(1, docs.len());
assert!(docs[0].body.is_unloaded());
# Ok(())
# }
```

## Updating

Updating a deferred field does not require it to be loaded. The
caller already supplies the value, so the field is loaded with the new
value after the update. No `.include()` is needed to read what was just
written:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     body: toasty::Deferred<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let created = toasty::create!(Document {
#     title: "Hello",
#     body: "old body",
# }).exec(&mut db).await?;
let mut doc = Document::filter_by_id(created.id).get(&mut db).await?;
assert!(doc.body.is_unloaded());

doc.update().body("new body".to_string()).exec(&mut db).await?;

// The field is loaded with the value just assigned.
assert_eq!("new body", doc.body.get());
# Ok(())
# }
```

## Optional deferred fields

`Deferred<T>` where `T` is `Option<U>` makes the field nullable. The
column stores `NULL` when the value is `None`, and `create!` treats the
field as optional:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct Document {
    #[key]
    #[auto]
    id: u64,

    title: String,
    summary: toasty::Deferred<Option<String>>,
}
```

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     summary: toasty::Deferred<Option<String>>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// summary may be set or omitted at create time.
let with = toasty::create!(Document {
    title: "With summary",
    summary: "a brief summary",
}).exec(&mut db).await?;

let without = toasty::create!(Document {
    title: "No summary",
}).exec(&mut db).await?;

let with = Document::filter_by_id(with.id)
    .include(Document::fields().summary())
    .get(&mut db)
    .await?;
assert_eq!(&Some("a brief summary".to_string()), with.summary.get());

let without = Document::filter_by_id(without.id)
    .include(Document::fields().summary())
    .get(&mut db)
    .await?;
assert_eq!(&None, without.summary.get());
# Ok(())
# }
```

A required `Deferred<T>` (where `T` is not `Option<_>`) is a required
argument to `create!`, just like any other non-nullable field —
`create!` fails to compile when it is missing.

## Driver support

Deferred fields are supported on every driver. SQL backends shorten the
`SELECT` column list; DynamoDB shortens the `ProjectionExpression`.
Drivers do not need a capability flag for this feature.
