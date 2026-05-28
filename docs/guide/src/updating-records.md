# Updating Records

To change field values on a record, use the `toasty::update!` macro.
The macro takes a target — a loaded model instance or a query that
selects which records to change — and a list of field assignments. It
returns a builder; call `.exec(&mut db)` to execute the update.

To load an instance to update, see [Querying Records](./querying-records.md).
To insert new records, see [Creating Records](./creating-records.md).

## Updating an instance

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[unique]
#     email: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let mut user = toasty::create!(User {
    name: "Alice",
    email: "alice@example.com",
})
.exec(&mut db)
.await?;

toasty::update!(user { name: "Alice Smith" })
    .exec(&mut db)
    .await?;

assert_eq!(user.name, "Alice Smith");
# Ok(())
# }
```

The instance reflects the new values after `.exec()`. Fields not
named in the macro keep their current values.

The generated SQL:

```sql
UPDATE users SET name = 'Alice Smith' WHERE id = 1;
```

## Updating multiple fields

List each field in the brace block:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[unique]
#     email: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut user = toasty::create!(User { name: "Alice", email: "alice@example.com" })
#     .exec(&mut db).await?;
toasty::update!(user {
    name: "Alice Smith",
    email: "alice.smith@example.com",
})
.exec(&mut db)
.await?;
# Ok(())
# }
```

## Field shorthand

When a local variable has the same name as a field, write the name
alone — the same shorthand Rust struct literals use:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut user = toasty::create!(User { name: "Alice" })
#     .exec(&mut db).await?;
let name = "Alice Smith";
toasty::update!(user { name })
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

## Modifying `Vec<scalar>` fields

Collection mutations live in `toasty::stmt`. The macro reaches them
as method calls on the field:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Article {
#     #[key]
#     #[auto]
#     id: u64,
#     tags: Vec<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut article = toasty::create!(Article { tags: vec!["rust".to_string()] })
#     .exec(&mut db).await?;
toasty::update!(article { tags.push("toasty") })
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

`tags.push("toasty")` lowers to `tags: toasty::stmt::push("toasty")`.
The same syntax reaches every function in `toasty::stmt`:
`tags.extend([...])`, `tags.pop()`, `tags.clear()`,
`tags.remove("x")`. See [`Vec<scalar>` Fields](./vec-scalar-fields.md#updating)
for the full list and per-driver support.

## Relative numeric updates

Arithmetic builders in `toasty::stmt` update a numeric field relative
to its stored value — incrementing a counter, crediting a balance.
The macro reaches them as method calls on the field, the same shape
as the collection mutations above:

| Method | What it does |
|---|---|
| `field.increment()` | Add `1` to the field. |
| `field.decrement()` | Subtract `1` from the field. |
| `field.add(value)` | Add `value` to the field. |
| `field.subtract(value)` | Subtract `value` from the field. |

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Account {
#     #[key]
#     #[auto]
#     id: u64,
#     balance: i64,
#     login_count: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut account = toasty::create!(Account { balance: 1000, login_count: 0 })
#     .exec(&mut db).await?;
// Credit an account by 100.
toasty::update!(account { balance.add(100) })
    .exec(&mut db)
    .await?;

// Debit by 25.
toasty::update!(account { balance.subtract(25) })
    .exec(&mut db)
    .await?;

// Bump a login counter.
toasty::update!(account { login_count.increment() })
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

Each operation is atomic against the existing column value — the
database applies it to whatever the row currently holds, not to a
client-side snapshot. Reading, modifying, and writing back from Rust
is a three-step round trip a concurrent writer can interleave,
dropping one of two updates on the same row; the arithmetic builders
fold the read and write into one statement.

These builders work on every backend and on every primitive numeric
type Toasty supports.

## Updating embedded fields

A brace block on the right side of `field:` updates the named
sub-fields of an [embedded struct](./embedded-types.md):

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Embed)]
# struct Metadata {
#     version: i64,
#     status: String,
# }
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     meta: Metadata,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut doc = toasty::create!(Document {
#     title: "Doc",
#     meta: Metadata { version: 1, status: "draft".to_string() },
# }).exec(&mut db).await?;
toasty::update!(doc {
    meta: { version: 2, status: "published" },
})
.exec(&mut db)
.await?;
# Ok(())
# }
```

Sub-fields not listed keep their values. Brace blocks nest for
embedded types within embedded types.

To replace an embedded value wholesale, pass the typed value:

```rust,ignore
toasty::update!(doc {
    meta: Metadata { version: 2, status: "published".into() },
})
.exec(&mut db).await?;
```

## Inserting has-many children

A bracket-of-braces literal on a [has-many](./has-many.md) field
inserts new children. Each `{ ... }` is a create builder for the
child model:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_many]
#     todos: toasty::Deferred<Vec<Todo>>,
# }
# #[derive(Debug, toasty::Model)]
# struct Todo {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     #[index]
#     user_id: u64,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::Deferred<User>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut user = User::create().name("Alice").exec(&mut db).await?;
toasty::update!(user {
    todos: [{ title: "buy milk" }, { title: "walk dog" }],
})
.exec(&mut db)
.await?;
# Ok(())
# }
```

Items can mix brace-block builders with `stmt::*` values:

```rust,ignore
toasty::update!(user {
    todos: [
        { title: "new todo" },
        toasty::stmt::remove(&old_todo),
    ],
})
.exec(&mut db).await?;
```

## Updating by query

`update!` accepts any query builder as a target. The update applies
to every record the query matches:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     name: String,
#     #[unique]
#     email: String,
# }
# async fn __example(mut db: toasty::Db, user_id: u64) -> toasty::Result<()> {
toasty::update!(User::filter_by_id(user_id) { name: "Bob" })
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

Scoped queries like `user.todos().filter_by_done(false)` are query
builders too. The query runs as part of the `UPDATE` statement; no
records are loaded first.

## Update by indexed field

Toasty generates `update_by_*` convenience methods for key and
indexed fields:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[unique]
#     email: String,
# }
# async fn __example(mut db: toasty::Db, user_id: u64) -> toasty::Result<()> {
User::update_by_id(user_id)
    .name("Bob")
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

`User::update_by_id(user_id)` is shorthand for
`User::filter_by_id(user_id).update()`. Toasty generates `update_by_*`
for each field marked `#[key]`, `#[unique]`, or `#[index]`.

## Setting an optional field to `None`

Pass `None` with the field's type annotation:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     bio: Option<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut user = toasty::create!(User { name: "Alice", bio: "Likes Rust" })
#     .exec(&mut db).await?;
toasty::update!(user { bio: Option::<String>::None })
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

## Concurrency control

When the model has a [`#[version]`](./concurrency-control.md) field,
instance updates condition the write on the version the instance
was loaded with and increment it atomically. If a concurrent writer
has modified the record, `.exec()` returns an error.

## Building updates programmatically

The macro lists every field at the macro call site. To set fields
based on a runtime condition, use the update builder directly:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     bio: Option<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut user = toasty::create!(User { name: "Alice" }).exec(&mut db).await?;
let mut builder = user.update().name("Alice Smith");

if true /* some condition */ {
    builder = builder.bio("Likes Rust");
}

builder.exec(&mut db).await?;
# Ok(())
# }
```

The chain form is also available for users who prefer it:

```rust,ignore
user.update()
    .name("Alice Smith")
    .exec(&mut db)
    .await?;
```

## What `#[derive(Model)]` generates

For a model `User` with `#[key]` on `id` and `#[unique]` on `email`,
the derive generates:

- `user.update()` — instance method (takes `&mut self`), returns an
  update builder. After `.exec()`, the instance reloads with the new
  values.
- `User::update_by_id(id)` — returns an update builder for the
  record matching the given key.
- `User::update_by_email(email)` — returns an update builder for the
  record matching the given email.
- `.update()` on any query builder — converts the query into an
  update builder.

The update builder has a setter for each field. The `UPDATE`
statement includes only the fields you set.
