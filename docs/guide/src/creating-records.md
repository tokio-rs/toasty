# Creating Records

Toasty generates a create builder for each model. Call `Model::create()`, set
fields with chained methods, and call `.exec(&mut db)` to insert the record.

## Creating a single record

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
let user = User::create()
    .name("Alice")
    .email("alice@example.com")
    .exec(&mut db)
    .await?;

println!("Created user with id: {}", user.id);
# Ok(())
# }
```

`User::create()` returns a builder with a setter method for each non-auto field.
Chain the setters to provide values, then call `.exec(&mut db)` to insert the
row and return the created `User` instance. Auto-generated fields like `id` are
populated on the returned value.

The generated SQL looks like:

```sql
INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');
```

## Required vs optional fields

Required fields (`String`, `u64`, etc.) must be set before calling `.exec()`.
Optional fields (`Option<T>`) default to `NULL` if not set.

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,
    bio: Option<String>,
}
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {

// bio defaults to None (NULL in the database)
let user = User::create()
    .name("Alice")
    .exec(&mut db)
    .await?;

assert!(user.bio.is_none());

// Or set it explicitly
let user = User::create()
    .name("Bob")
    .bio("Likes Rust")
    .exec(&mut db)
    .await?;

assert_eq!(user.bio.as_deref(), Some("Likes Rust"));
# Ok(())
# }
```

## Creating many records

`Model::create_many()` inserts multiple records at once. Add items with `.item()`
or `.with_item()`, then call `.exec()` to insert them all. It returns a `Vec` of
the created instances.

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
let users = User::create_many()
    .item(User::create().name("Alice").email("alice@example.com"))
    .item(User::create().name("Bob").email("bob@example.com"))
    .exec(&mut db)
    .await?;

assert_eq!(users.len(), 2);
# Ok(())
# }
```

The `.with_item()` variant takes a closure that receives the create builder:

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
let users = User::create_many()
    .with_item(|u| u.name("Alice").email("alice@example.com"))
    .with_item(|u| u.name("Bob").email("bob@example.com"))
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

Both forms are equivalent. Use whichever reads better for your use case.

## Nested creation

When models have relationships, you can create a parent and its children in a
single call. This is covered in more detail in the relationship chapters
([BelongsTo](./belongs-to.md), [HasMany](./has-many.md), [HasOne](./has-one.md)),
but here is a preview.

Given a `User` that has many `Todo` items:

```rust,ignore
let user = User::create()
    .name("Alice")
    .todo(Todo::create().title("Buy groceries"))
    .todo(Todo::create().title("Write docs"))
    .exec(&mut db)
    .await?;
```

Toasty creates the user first, then creates each todo with the user's ID
automatically set as the foreign key. All records are created within the same
operation.

## What gets generated

For a model like `User`, `#[derive(Model)]` generates:

- `User::create()` — returns a builder with a setter for each non-auto field
- `User::create_many()` — returns a bulk-insert builder with `.item()` and
  `.with_item()` methods

The create builder's setter methods accept flexible input types through the
`IntoExpr` trait. For a `String` field, you can pass `&str`, `String`, or
`&String`. For numeric fields, you can pass the value directly or by reference.
See [Defining Models — What types can you pass to setters?](./defining-models.md#what-types-can-you-pass-to-setters)
for details.
