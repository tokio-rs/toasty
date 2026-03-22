# Creating Records

Toasty generates a create builder for each model. Call `<YourModel>::create()`,
set fields with chained methods, and call `.exec(&mut db)` to insert the record.

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
row. The returned `User` instance has all fields set, including auto-generated
ones like `id`.

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

Use `toasty::stmt::batch()` to insert multiple records at once. Pass an array of
create builders for the same model, or a tuple for mixed models. Call `.exec()`
to insert them all.

### Array syntax (same model)

When creating multiple records of the same model, pass an array. The return type
is `Vec<User>`:

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
let users = toasty::stmt::batch([
    User::create().name("Alice").email("alice@example.com"),
    User::create().name("Bob").email("bob@example.com"),
    User::create().name("Carol").email("carol@example.com"),
])
.exec(&mut db)
.await?;

assert_eq!(users.len(), 3);
# Ok(())
# }
```

This also works with a `Vec` of builders, which is useful when the number of
records is determined at runtime:

```rust,ignore
let names = ["Alice", "Bob", "Carol"];
let builders: Vec<_> = names
    .iter()
    .enumerate()
    .map(|(i, n)| User::create().name(*n).email(format!("user{i}@example.com")))
    .collect();

let users = toasty::stmt::batch(builders).exec(&mut db).await?;
```

### Tuple syntax (mixed models)

When creating records of different models, use a tuple. The return type matches
the tuple structure:

```rust,ignore
let (user, post): (User, Post) = toasty::stmt::batch((
    User::create().name("Alice"),
    Post::create().title("Hello World"),
))
.exec(&mut db)
.await?;
```

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
automatically set as the foreign key. Toasty makes a best effort to execute
nested creation atomically — either all records are inserted or none are.
Whether full atomicity is guaranteed depends on your database's capabilities, so
check your database's transaction and consistency documentation.

## What gets generated

For a model like `User`, `#[derive(Model)]` generates:

- `User::create()` — returns a builder with a setter for each non-auto field

The create builder's setter methods accept flexible input types through the
`IntoExpr` trait. For a `String` field, you can pass `&str`, `String`, or
`&String`. For numeric fields, you can pass the value directly or by reference.
See [Defining Models — What types can you pass to setters?](./defining-models.md#what-types-can-you-pass-to-setters)
for details.
