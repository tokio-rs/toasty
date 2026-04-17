# Deleting Records

Toasty provides several ways to delete records: from an instance, by primary
key, or through a query.

## Deleting an instance

Call `.delete()` on a model instance, then `.exec(&mut db)`:

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
let user = toasty::create!(User {
    name: "Alice",
    email: "alice@example.com",
})
.exec(&mut db)
.await?;

let user_id = user.id;

user.delete().exec(&mut db).await?;

// The record no longer exists
let result = User::get_by_id(&mut db, &user_id).await;
assert!(result.is_err());
# Ok(())
# }
```

The `.delete()` method consumes the instance (takes `self`, not `&self`). After
deleting, you can no longer use the instance.

If the model has a [`#[version]`](./field-options.md#optimistic-concurrency-with-version)
field, instance deletes are version-guarded: Toasty conditions the deletion on the
version the instance was last loaded with. If a concurrent writer has modified the
record in the meantime, `.exec()` returns an error.

The generated SQL looks like:

```sql
DELETE FROM users WHERE id = 1;
```

## Deleting by primary key

Use the generated `delete_by_*` method to delete a record by its key without
loading it first:

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
# let user = toasty::create!(User { name: "Alice", email: "alice@example.com" })
#     .exec(&mut db)
#     .await?;
User::delete_by_id(&mut db, user.id).await?;
# Ok(())
# }
```

This executes the delete directly — no `SELECT` query is issued first.

## Deleting by query

Build a query and call `.delete()` on it to delete all matching records:

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
# toasty::create!(User { name: "Alice", email: "alice@example.com" })
#     .exec(&mut db)
#     .await?;
User::filter_by_email("alice@example.com")
    .delete()
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

You can use any query builder — `filter_by_*`, `filter()`, or `all()` — and
chain `.delete()` to convert it into a delete operation.

## What gets generated

For a model with `#[key]` on `id` and `#[unique]` on `email`, Toasty generates:

- `user.delete()` — instance method (consumes `self`), returns a delete
  statement. Call `.exec(&mut db)` to execute.
- `User::delete_by_id(&mut db, id)` — deletes the record matching the given key.
  Executes immediately.
- `User::delete_by_email(&mut db, email)` — deletes the record matching the
  given email. Executes immediately.
- Any query builder's `.delete()` method — converts the query into a delete
  statement.
