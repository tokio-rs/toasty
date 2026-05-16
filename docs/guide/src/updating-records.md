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
replacement through the setter and a set of incremental builders —
`stmt::push`, `stmt::extend`, `stmt::pop`, `stmt::remove`,
`stmt::remove_at`, `stmt::clear`, and `stmt::apply`. See
[`Vec<scalar>` Fields](./vec-scalar-fields.md#updating) for the full
treatment, including which builders each driver supports.

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
