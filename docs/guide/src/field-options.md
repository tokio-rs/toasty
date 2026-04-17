# Field Options

Toasty provides several field-level attributes to control how fields map to
database columns: custom column names, explicit types, default values, update
expressions, and JSON serialization.

## Custom column names

By default, a Rust field name maps directly to a column name. Use
`#[column("name")]` to override this:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    #[column("display_name")]
    name: String,
}
```

The field is still accessed as `user.name` in Rust, but the database column is
named `display_name`:

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    display_name TEXT NOT NULL
);
```

## Explicit column types

Toasty infers the column type from the Rust field type. Use
`#[column(type = ...)]` to specify an explicit database type instead:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    #[column(type = varchar(100))]
    name: String,
}
```

This creates a `VARCHAR(100)` column instead of `TEXT`. The database rejects
values that exceed the specified length.

You can combine a custom name with an explicit type:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    #[column("display_name", type = varchar(100))]
    name: String,
}
```

Supported type values:

| Type syntax | Database type |
|---|---|
| `boolean` | Boolean |
| `int`, `i8`, `i16`, `i32`, `i64` | Integer (various sizes) |
| `uint`, `u8`, `u16`, `u32`, `u64` | Unsigned integer |
| `text` | Text |
| `varchar(N)` | Variable-length string with max length |
| `numeric`, `numeric(P, S)` | Decimal with optional precision and scale |
| `binary(N)`, `blob` | Binary data |
| `timestamp(P)` | Timestamp with precision |
| `date` | Date |
| `time(P)` | Time with precision |
| `datetime(P)` | Date and time with precision |

Not all databases support all column types. Toasty validates explicit column
types against the database's capabilities when you call `db.push_schema()`. If a
type is not supported, schema creation fails with an error. For example,
`varchar` is supported by PostgreSQL and MySQL but not by SQLite or DynamoDB —
using `#[column(type = varchar(100))]` with SQLite produces an error like
`"unsupported feature: VARCHAR type is not supported by this database"`. If the
requested size exceeds the database's maximum, Toasty reports that as well.

## Default values

Use `#[default(expr)]` to set a default value applied when creating a record.
If you don't set the field on the create builder, Toasty evaluates the
expression and uses the result.

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#[default(0)]
view_count: i64,
# }
```

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     #[default(0)]
#     view_count: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// view_count defaults to 0
let post = toasty::create!(Post { title: "Hello World" })
    .exec(&mut db)
    .await?;

assert_eq!(post.view_count, 0);

// Override the default by setting it explicitly
let post = toasty::create!(Post {
    title: "Popular Post",
    view_count: 100,
})
.exec(&mut db)
.await?;

assert_eq!(post.view_count, 100);
# Ok(())
# }
```

The expression inside `#[default(...)]` is any valid Rust expression. It runs at
insert time, not at compile time.

`#[default]` only applies on create. It has no effect on updates.

## Update expressions

Use `#[update(expr)]` to set an expression that applies on both create and
update. Each time the record is created or updated, Toasty evaluates the
expression and sets the field — unless you explicitly override it.

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#[update(jiff::Timestamp::now())]
updated_at: jiff::Timestamp,
# }
```

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     #[update(jiff::Timestamp::now())]
#     updated_at: jiff::Timestamp,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// updated_at is set automatically on create
let mut post = toasty::create!(Post { title: "Hello World" })
    .exec(&mut db)
    .await?;

// updated_at is refreshed automatically on update
post.update()
    .title("Updated Title")
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

You can override the automatic value by setting the field explicitly:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     #[update(jiff::Timestamp::now())]
#     updated_at: jiff::Timestamp,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut post = toasty::create!(Post { title: "Hello World" })
#     .exec(&mut db)
#     .await?;
let explicit_ts = jiff::Timestamp::from_second(946684800).unwrap();
post.update()
    .title("Backdated")
    .updated_at(explicit_ts)
    .exec(&mut db)
    .await?;

assert_eq!(post.updated_at, explicit_ts);
# Ok(())
# }
```

### Combining `#[default]` and `#[update]`

You can use both attributes on the same field. `#[default]` applies on create,
`#[update]` applies on update:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
// On create: "draft". On update: "edited".
#[default("draft".to_string())]
#[update("edited".to_string())]
status: String,
# }
```

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     #[default("draft".to_string())]
#     #[update("edited".to_string())]
#     status: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let mut post = toasty::create!(Post { title: "Hello" })
    .exec(&mut db)
    .await?;

assert_eq!(post.status, "draft");

post.update().title("Updated").exec(&mut db).await?;

assert_eq!(post.status, "edited");
# Ok(())
# }
```

## Optimistic concurrency with `#[version]`

Use `#[version]` on a `u64` field to enable optimistic concurrency control (OCC)
for a model. The version field is managed entirely by Toasty — you declare it but
never set it manually.

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct Document {
    #[key]
    #[auto]
    id: uuid::Uuid,

    content: String,

    #[version]
    version: u64,
}
```

**Create:** the version field is initialized to `1` on the newly created record.

**Instance update:** when you call `doc.update()...exec()`, Toasty conditions the
write on the current version value and atomically increments it. If another writer
has already updated the record since you last loaded it, the update returns an
error.

**Instance delete:** `doc.delete().exec()` conditions the deletion on the current
version. If the record has been updated since you last loaded it, the delete
returns an error.

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: uuid::Uuid,
#     content: String,
#     #[version]
#     version: u64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let mut doc = toasty::create!(Document { content: "hello" })
    .exec(&mut db)
    .await?;

assert_eq!(doc.version, 1);

// Load a second handle — both start at version 1
let mut stale = Document::get_by_id(&mut db, &doc.id).await?;

// Advance doc to version 2
doc.update().content("world").exec(&mut db).await?;
assert_eq!(doc.version, 2);

// stale is still at version 1 — update fails with a conflict error
let result = stale.update().content("conflict").exec(&mut db).await;
assert!(result.is_err());
# Ok(())
# }
```

Query-based updates (`Document::filter_by_id(id).update()...`) do not check or
increment the version — OCC only applies to instance updates and instance deletes.

> **Note:** `#[version]` is currently supported by the DynamoDB driver only. SQL
> drivers do not yet implement OCC.

## Timestamps with `#[auto]`

For timestamp fields named `created_at` or `updated_at`, `#[auto]` provides a
shorthand:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: u64,

    title: String,

    #[auto]
    created_at: jiff::Timestamp,

    #[auto]
    updated_at: jiff::Timestamp,
}
```

When `#[auto]` appears without arguments on a non-key field, Toasty uses a
heuristic based on the field name and type to determine the behavior:

| Field name | Field type | `#[auto]` expands to |
|---|---|---|
| `created_at` | `jiff::Timestamp` | `#[default(jiff::Timestamp::now())]` — set once on create |
| `updated_at` | `jiff::Timestamp` | `#[update(jiff::Timestamp::now())]` — refreshed on every create and update |

On key fields, bare `#[auto]` defers to the type's default auto-generation
strategy (e.g., auto-increment for integers, UUID v7 for `uuid::Uuid`). See
[Keys and Auto-Generation](./keys-and-auto-generation.md) for details.

This is the recommended way to add timestamps to your models. The `created_at`
field is set when the record is first inserted and never changes. The
`updated_at` field is refreshed each time the record is updated.

Timestamp fields require the `jiff` feature:

```toml
[dependencies]
toasty = { version = "{{toasty_version}}", features = ["sqlite", "jiff"] }
```

## Date and time fields

With the `jiff` feature enabled, you can use these types for date and time
fields:

| Rust type | Description |
|---|---|
| `jiff::Timestamp` | An instant in time (UTC) |
| `jiff::civil::Date` | A date without time |
| `jiff::civil::Time` | A time of day without date |
| `jiff::civil::DateTime` | A date and time without timezone |

You can control the storage precision with `#[column(type = ...)]`:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct Event {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[column(type = timestamp(3))]
    starts_at: jiff::Timestamp,

    #[column(type = time(0))]
    reminder_time: jiff::civil::Time,
}
```

## JSON serialization

Use `#[serialize(json)]` to store a Rust value as a JSON string in the database.
The field type must implement `serde::Serialize` and `serde::Deserialize`.

```rust,ignore
# use toasty::Model;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    #[serialize(json)]
    tags: Vec<String>,

    #[serialize(json)]
    meta: Metadata,
}
```

Toasty serializes the value to a JSON string on insert and update, and
deserializes it back when reading. The default database column type is `TEXT`.
You can override this with `#[column(type = ...)]` if needed — for example,
`#[column(type = varchar(1000))]` to limit the stored JSON size on databases
that support `varchar`.

```rust,ignore
# use toasty::Model;
# use serde::{Serialize, Deserialize};
# #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
# struct Metadata {
#     version: u32,
#     labels: Vec<String>,
# }
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     #[serialize(json)]
#     tags: Vec<String>,
#     #[serialize(json)]
#     meta: Metadata,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let post = toasty::create!(Post {
    title: "Hello",
    tags: vec!["rust".to_string(), "toasty".to_string()],
    meta: Metadata {
        version: 1,
        labels: vec!["alpha".to_string()],
    },
})
.exec(&mut db)
.await?;

assert_eq!(post.tags, vec!["rust", "toasty"]);
assert_eq!(post.meta.version, 1);
# Ok(())
# }
```

### Nullable JSON fields

By default, `#[serialize(json)]` creates a `NOT NULL` column. An `Option<T>`
field with `#[serialize(json)]` serializes `None` as the JSON text `"null"` —
the column still stores a non-null string.

To allow SQL `NULL` in the column, add the `nullable` modifier:

```rust
# use toasty::Model;
# use std::collections::HashMap;
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#[serialize(json, nullable)]
metadata: Option<HashMap<String, String>>,
# }
```

With `nullable`:
- `None` maps to SQL `NULL` in the database
- `Some(value)` maps to the JSON string representation

Without `nullable`:
- `None` maps to the JSON text `"null"` (a non-null string)
- `Some(value)` maps to the JSON string representation

## Attribute summary

| Attribute | Purpose | Applies on |
|---|---|---|
| `#[column("name")]` | Custom column name | — |
| `#[column(type = ...)]` | Explicit column type | — |
| `#[default(expr)]` | Default value | Create only |
| `#[update(expr)]` | Automatic value | Create and update |
| `#[auto]` on `created_at` | Shorthand for `#[default(jiff::Timestamp::now())]` | Create only |
| `#[auto]` on `updated_at` | Shorthand for `#[update(jiff::Timestamp::now())]` | Create and update |
| `#[serialize(json)]` | Store as JSON text | Create and update |
| `#[serialize(json, nullable)]` | Store as JSON text with SQL NULL support | Create and update |
| `#[version]` | Optimistic concurrency control (DynamoDB only) | Create and update |
