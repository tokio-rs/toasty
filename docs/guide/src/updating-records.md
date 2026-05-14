# Updating Records

Toasty generates an update builder for each model. You can update a record
through an instance method, through a query, or with a generated convenience
method.

## Updating an instance

Call `.update()` on a mutable model instance, set the fields you want to change,
and call `.exec(&mut db)`:

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

user.update()
    .name("Alice Smith")
    .exec(&mut db)
    .await?;

// The instance is updated in place
assert_eq!(user.name, "Alice Smith");
# Ok(())
# }
```

Only set the fields you want to change. Fields you don't set keep their current
values. The `.update()` method takes `&mut self`, so you need a mutable binding.
After `.exec()` completes, the instance reflects the new values.

If the model has a [`#[version]`](./concurrency-control.md) field, instance
updates are version-guarded: Toasty conditions the write on the version the
instance was last loaded with and increments it atomically. If a concurrent
writer has modified the record in the meantime, `.exec()` returns an error.

The generated SQL looks like:

```sql
UPDATE users SET name = 'Alice Smith' WHERE id = 1;
```

## Updating multiple fields

Chain multiple setters to update several fields at once:

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
#     .exec(&mut db)
#     .await?;
user.update()
    .name("Alice Smith")
    .email("alice.smith@example.com")
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

## Updating by query

You can update records without first loading them by building a query and calling
`.update()` on it:

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
User::filter_by_id(user_id)
    .update()
    .name("Bob")
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

This executes the update directly without loading the record first. The query
determines which records to update, and the chained setters specify the new
values.

## Update by indexed field

Toasty generates `update_by_*` convenience methods for key and indexed fields:

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

This is shorthand for `User::filter_by_id(user_id).update()`. Toasty generates
`update_by_*` for each field that has `#[key]`, `#[unique]`, or `#[index]`.

## Setting optional fields to `None`

To clear an optional field, pass `None` with the appropriate type annotation:

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
#     .exec(&mut db)
#     .await?;
user.update()
    .bio(Option::<String>::None)
    .exec(&mut db)
    .await?;

assert!(user.bio.is_none());
# Ok(())
# }
```

## Modifying a `Vec<scalar>` field

A `Vec<scalar>` field (e.g. `tags: Vec<String>`) supports whole-value
replacement through the setter:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Item {
#     #[key]
#     #[auto]
#     id: u64,
#     tags: Vec<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut item = toasty::create!(Item { tags: vec!["a".to_string()] })
#     .exec(&mut db)
#     .await?;
item.update()
    .tags(vec!["x".to_string(), "y".to_string()])
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

For incremental mutations, the `toasty::stmt` module provides builders
that produce one statement per call and refresh the instance field in
place:

| Function | What it does |
|---|---|
| `stmt::push(value)` | Append one element |
| `stmt::extend(iter)` | Append every element of an iterator, in order |
| `stmt::pop()` | Remove the last element |
| `stmt::remove(value)` | Remove every element equal to the value |
| `stmt::remove_at(idx)` | Remove the element at a 0-based index |
| `stmt::clear()` | Replace the field with an empty list |

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Item {
#     #[key]
#     #[auto]
#     id: u64,
#     tags: Vec<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut item = toasty::create!(Item { tags: vec!["a".to_string()] })
#     .exec(&mut db)
#     .await?;
// Append one element.
item.update()
    .tags(toasty::stmt::push("admin"))
    .exec(&mut db)
    .await?;

// Append several. `stmt::extend(empty)` is a no-op.
item.update()
    .tags(toasty::stmt::extend(["verified", "staff"]))
    .exec(&mut db)
    .await?;

// Remove the last element.
item.update()
    .tags(toasty::stmt::pop())
    .exec(&mut db)
    .await?;

// Remove every element equal to "staff".
item.update()
    .tags(toasty::stmt::remove("staff"))
    .exec(&mut db)
    .await?;

// Remove the element at index 0.
item.update()
    .tags(toasty::stmt::remove_at(0usize))
    .exec(&mut db)
    .await?;

// Remove every element.
item.update()
    .tags(toasty::stmt::clear())
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

`push`, `extend`, and `clear` work on every backend. Each append is
atomic against the existing column value: PostgreSQL uses `text[]`
concatenation, MySQL uses `JSON_MERGE_PRESERVE`, SQLite reads and
re-emits the JSON array in one statement, and DynamoDB uses
`list_append`.

`pop`, `remove`, and `remove_at` currently require PostgreSQL, where
they lower to `array_remove` and array slicing. Other backends return
an error. `pop` on an empty list and `remove_at` past the end of the
list are no-ops; `remove` deletes every matching element, not just the
first.

After `.exec()`, the instance's field reflects the new value.
Concurrent writers can still interleave — the next operation sees what
the database has, not what the local instance holds — but each
individual operation is indivisible at the storage layer.

## What gets generated

For a model with `#[key]` on `id` and `#[unique]` on `email`, Toasty generates:

- `user.update()` — instance method (takes `&mut self`), returns an update
  builder. After `.exec()`, the instance is reloaded with the new values.
- `User::update_by_id(id)` — returns an update builder for the record matching
  the given key.
- `User::update_by_email(email)` — returns an update builder for the record
  matching the given email.
- Any query builder's `.update()` method — converts the query into an update
  builder.

The update builder has a setter method for each field. Only the fields you set
are included in the `UPDATE` statement.
