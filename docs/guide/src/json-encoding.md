# JSON Encoding

`toasty::Json<T>` stores a serde-compatible Rust value in one database
column. Toasty serializes `T` before the query engine processes the value
and deserializes it after the driver reads the column.

Use JSON encoding for data that the application reads and writes as a
whole. Toasty does not expose typed paths into a `Json<T>` field because
the query engine sees the encoded value as a string rather than as the
fields of `T`.

## Choosing a field representation

Toasty has three ways to store structured or collection data in one
field:

| Rust field | Use it for | Query support |
|---|---|---|
| `toasty::Json<T>` | Any `T` with serde serialization | No typed paths into `T` |
| `#[document]` on an embedded struct | A fixed object whose fields Toasty knows | Filters on scalar fields inside the object |
| `Vec<T>` where `T` is a scalar | A homogeneous list | Collection predicates and incremental mutations |

[`#[document]` Fields](./document-fields.md) explains schema-aware
document storage. [`Vec<scalar>` Fields](./vec-scalar-fields.md) covers
typed lists. Use `Json<T>` when generated paths and collection operators
are not required or when `T` cannot derive `toasty::Embed`.

## Enabling JSON encoding

Enable Toasty's `serde` feature together with the selected database
driver. The application also needs `serde` for typed payloads and
`serde_json` when it constructs dynamic JSON values:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toasty = { version = "{{toasty_version}}", features = ["postgresql", "serde"] }
```

The inner `T` in `Json<T>` must implement `serde::Serialize` and
`serde::Deserialize`.

## Encoding a typed payload

Wrap the model field in `toasty::Json<T>` and select a column type with
`#[column(type = ...)]`:

```rust,ignore
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Metadata {
    version: u32,
    labels: Vec<String>,
}

#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: u64,

    title: String,

    #[column(type = json)]
    metadata: toasty::Json<Metadata>,
}
```

Every `Json<T>` field requires an explicit column type. The Rust type
determines how Toasty encodes and decodes the value; the column
attribute determines how the database stores the encoded JSON.

## Column types

JSON fields accept these column types:

| Column type | Database representation |
|---|---|
| `text` | JSON text in the backend's string type |
| `varchar(N)` | Length-limited JSON text on backends with native `VARCHAR` |
| `json` | Native JSON on PostgreSQL and MySQL |
| `jsonb` | Native binary JSON on PostgreSQL |

Use `text` on SQLite, Turso, DynamoDB, or any backend where the database
does not provide a native JSON column. Toasty's serializer still emits
valid JSON, but a text column does not ask the database to validate
external writes.

Use `json` when PostgreSQL or MySQL should validate the stored value as
JSON. Use `jsonb` on PostgreSQL when its binary representation and JSONB
operators are required outside Toasty's typed query API.

Toasty checks `json` and `jsonb` against the selected driver's
capabilities during schema construction. A model that requests `jsonb`
on MySQL or SQLite fails before Toasty sends unsupported DDL.

## Creating and updating values

Create and update setters accept the inner `T`, so callers do not need
to construct `Json(T)`:

```rust,ignore
let mut post = toasty::create!(Post {
    title: "Encoding data",
    metadata: Metadata {
        version: 1,
        labels: vec!["rust".to_string()],
    },
})
.exec(&mut db)
.await?;

post.update()
    .metadata(Metadata {
        version: 2,
        labels: vec!["rust".to_string(), "orm".to_string()],
    })
    .exec(&mut db)
    .await?;
```

The loaded model retains the wrapper. `Json<T>` implements `Deref` and
`AsRef`, so code can read the inner value without moving it:

```rust,ignore
assert_eq!(post.metadata.version, 2);
let metadata: &Metadata = post.metadata.as_ref();
```

An update replaces the entire encoded value. Toasty does not generate
an assignment for one field inside `T` because `T` has no field paths in
the statement schema.

## Dynamic JSON values

Use `serde_json::Value` directly when the application needs a dynamic
JSON object or array:

```rust,ignore
#[derive(Debug, toasty::Model)]
struct Event {
    #[key]
    #[auto]
    id: u64,

    #[column(type = jsonb)]
    payload: serde_json::Value,
}
```

`serde_json::Value` uses the same encoding, storage types, and
whole-value update behavior as `Json<T>`. The wrapper would add no type
information because `serde_json::Value` already represents arbitrary
JSON.

```rust,ignore
let event = toasty::create!(Event {
    payload: serde_json::json!({
        "kind": "published",
        "article_id": 42,
        "tags": ["rust", "orm"],
    }),
})
.exec(&mut db)
.await?;
```

## SQL `NULL` and JSON `null`

The position of `Option` determines whether absence belongs to the
database column or to the JSON value:

| Field type | Rust value | Stored value |
|---|---|---|
| `Option<Json<T>>` | `None` | SQL `NULL` |
| `Json<Option<T>>` | `Json(None)` | JSON `null` in a non-null column |
| `Option<serde_json::Value>` | `None` | SQL `NULL` |
| `serde_json::Value` | `Value::Null` | JSON `null` in a non-null column |

Use `Option<Json<T>>` when the row may have no payload. Use
`Json<Option<T>>` when every row must contain JSON and `null` is part of
the payload's data model.

```rust,ignore
#[derive(Debug, toasty::Model)]
struct Import {
    #[key]
    #[auto]
    id: u64,

    #[column(type = json)]
    result: Option<toasty::Json<ImportResult>>,

    #[column(type = json)]
    external_value: serde_json::Value,
}
```

## Deferring a JSON field

Wrap the JSON field in `toasty::Deferred` when ordinary queries should
omit a large payload:

```rust,ignore
#[column(type = jsonb)]
payload: toasty::Deferred<toasty::Json<Payload>>,
```

The field follows the normal deferred loading rules. A regular query
leaves it unloaded, `.include(Model::fields().payload())` adds the
column to that query, and assigning a new payload leaves the in-memory
field loaded with the assigned value. See [Deferred
Fields](./deferred-fields.md) for the loading API.

`toasty::Deferred<serde_json::Value>` is also supported and uses the
same explicit column types.

## Query architecture

A `Json<T>` field registers an application-level string type plus JSON
serialization metadata and the selected database storage type. Its
`IntoExpr` implementation serializes `T` into one string expression
before planning.

SQL drivers bind that string directly to `text`, `json`, or `jsonb`.
When a driver reads a native JSON column for a `Json<T>` or
`serde_json::Value` field, it returns the JSON text rather than a
structural Toasty value. DynamoDB stores text-backed JSON in an `S`
attribute. The field's `Load` implementation invokes `serde_json` to
reconstruct the Rust value.

This boundary makes arbitrary serde types possible without registering
their fields in Toasty's schema. It also explains why a path such as
`Post::fields().metadata().version()` does not exist: the query engine
knows the column and its storage type, but it does not know the shape of
`Metadata`.

Use [`#[document]`](./document-fields.md) when Toasty must retain the
object's field structure for filters. Use a relation when the nested
data needs independent keys, indexes, or lifecycle.

> **Runnable example:** [`cms-article-fields`] stores an SEO payload in
> `Json<T>` alongside a queryable `Vec<scalar>` and a deferred text
> column.

[`cms-article-fields`]: https://github.com/tokio-rs/toasty/tree/main/examples/cms-article-fields
