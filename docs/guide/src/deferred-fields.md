# Deferred Fields

A deferred field is a column Toasty omits from the default `SELECT`
list. Records returned from a query have the field unloaded; loading
the value requires either a follow-up `.exec()` call or a preload with
`.include()`.

The pattern fits columns that are large, expensive to fetch, or rarely
read: a `Document` body, a binary blob, an audit-event JSON payload.
Without the deferred annotation, every list query reads every column
whether the caller needs it or not.

The API mirrors `BelongsTo`: a synchronous `.get()` reads an
already-loaded value, an async per-field accessor loads on demand, and
`.include()` preloads as part of the parent query.

## Marking a field as deferred

Annotate the field with `#[deferred]` and wrap its type in `Deferred<T>`:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct Document {
    #[key]
    #[auto]
    id: u64,

    title: String,

    #[deferred]
    body: toasty::Deferred<String>,
}
```

Both are required; using one without the other is a compile error. The
attribute directs the macro to omit the field from the default
projection and to generate the per-field load method. The wrapper type
provides the unloaded-state runtime API.

`#[deferred]` is supported on primitive fields and on embedded types
(`#[derive(Embed)]`). It does not compose with `#[belongs_to]`,
`#[has_many]`, or `#[has_one]` — relations are already lazy.

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
#     #[deferred]
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

## Loading on demand

The macro generates a per-field method that issues a single-row read
keyed on the model's primary key. Call `.exec()` to fetch the value:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     #[deferred]
#     body: toasty::Deferred<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let created = toasty::create!(Document {
#     title: "Hello",
#     body: "the long body",
# }).exec(&mut db).await?;
# let doc = Document::filter_by_id(created.id).get(&mut db).await?;
let body: String = doc.body().exec(&mut db).await?;
# Ok(())
# }
```

The return type of `.exec()` is the type within `Deferred<T>`. The
call does not mutate the in-memory record — `doc.body.is_unloaded()`
is still `true` afterward, and re-issuing the same load returns the
value again.

The `.await` makes the round-trip explicit. Code that needs the value
many times should preload with `.include()` instead — calling `.exec()`
in a loop over a `Vec<Document>` is N+1 by definition.

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
#     #[deferred]
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
    .include(Document::fields().author())   // BelongsTo
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
#     #[deferred]
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
value after the update — no follow-up `.exec()` or `.include()` is
needed to read what was just written:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     #[deferred]
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

    #[deferred]
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
#     #[deferred]
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

let summary: Option<String> = with.summary().exec(&mut db).await?;
assert_eq!(Some("a brief summary".to_string()), summary);

let summary: Option<String> = without.summary().exec(&mut db).await?;
assert_eq!(None, summary);
# Ok(())
# }
```

A required `Deferred<T>` (where `T` is not `Option<_>`) is a required
argument to `create!`, just like any other non-nullable field —
`create!` fails to compile when it is missing.

## Driver support

`#[deferred]` is supported on every driver. SQL backends shorten the
`SELECT` column list; DynamoDB shortens the `ProjectionExpression`.
Drivers do not need a capability flag for this feature.
