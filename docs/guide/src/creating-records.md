# Creating Records

Toasty provides two ways to create records: the `toasty::create!` macro and
the create builder. The macro uses struct-literal syntax and expands to builder
calls under the hood. Most code uses the macro; the builder is there when you
need programmatic control (e.g., conditional fields).

## Creating a single record

With the macro:

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

println!("Created user with id: {}", user.id);
# Ok(())
# }
```

This expands to the equivalent builder code:

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
# Ok(())
# }
```

Both return a create builder. Call `.exec(&mut db)` to insert the row. The
returned `User` instance has all fields set, including auto-generated ones
like `id`.

The generated SQL looks like:

```sql
INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');
```

Field values in the macro can be any Rust expression — literals, variables, or
function calls:

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
let name = "Bob";
let user = toasty::create!(User {
    name: name,
    email: format!("{}@example.com", name.to_lowercase()),
})
.exec(&mut db)
.await?;
# Ok(())
# }
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
let user = toasty::create!(User { name: "Alice" })
    .exec(&mut db)
    .await?;

assert!(user.bio.is_none());

// Or set it explicitly
let user = toasty::create!(User {
    name: "Bob",
    bio: "Likes Rust",
})
.exec(&mut db)
.await?;

assert_eq!(user.bio.as_deref(), Some("Likes Rust"));
# Ok(())
# }
```

## Creating through a relation

Use the `in` keyword to create a record through a relation accessor. Toasty
sets the foreign key automatically:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_many]
#     todos: toasty::HasMany<Todo>,
# }
# #[derive(Debug, toasty::Model)]
# struct Todo {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     user_id: u64,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::BelongsTo<User>,
#     title: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let user = toasty::create!(User { name: "Alice" }).exec(&mut db).await?;
let todo = toasty::create!(in user.todos() { title: "Buy groceries" })
    .exec(&mut db)
    .await?;

assert_eq!(todo.user_id, user.id);
# Ok(())
# }
```

You don't need to set `user_id` — Toasty fills it in from the parent. The
macro expands to `user.todos().create().title("Buy groceries")`.

## Nested creation

When models have relationships, you can create a parent and its children in a
single call. Inside the macro, use `{ ... }` (without a type prefix) for
BelongsTo/HasOne fields, and `[{ ... }, { ... }]` for HasMany fields:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_many]
#     todos: toasty::HasMany<Todo>,
# }
# #[derive(Debug, toasty::Model)]
# struct Todo {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     user_id: u64,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::BelongsTo<User>,
#     title: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let user = toasty::create!(User {
    name: "Alice",
    todos: [{ title: "Buy groceries" }, { title: "Write docs" }],
})
.exec(&mut db)
.await?;

let todos = user.todos().exec(&mut db).await?;
assert_eq!(2, todos.len());
# Ok(())
# }
```

This expands to:

```rust,ignore
User::create()
    .name("Alice")
    .with_todos(|b| {
        b.with_item(|b| b.title("Buy groceries"))
         .with_item(|b| b.title("Write docs"))
    })
    .exec(&mut db)
    .await?;
```

Toasty creates the user first, then creates each todo with the user's ID
automatically set as the foreign key. Nesting works to arbitrary depth — a
nested record can itself contain nested records.

Toasty makes a best effort to execute nested creation atomically — either all
records are inserted or none are. Whether full atomicity is guaranteed depends
on your database's capabilities, so check your database's transaction and
consistency documentation.

The relationship chapters ([BelongsTo](./belongs-to.md),
[HasMany](./has-many.md), [HasOne](./has-one.md)) cover nested creation in
more detail.

## Creating many records

### Same-type batch

Use the `::[ ... ]` syntax to create multiple records of the same model:

```rust,ignore
let (alice, bob, carol) = toasty::create!(User::[
    { name: "Alice", email: "alice@example.com" },
    { name: "Bob", email: "bob@example.com" },
    { name: "Carol", email: "carol@example.com" },
])
.exec(&mut db)
.await?;
```

The same-type batch returns a tuple with one element per record. The batch
is atomic — all records are inserted together or none are.

### Mixed-type batch

Use `[ ... ]` to create records of different models in a single batch:

```rust,ignore
let (user, post) = toasty::create!([
    User { name: "Alice" },
    Post { title: "Hello World" },
])
.exec(&mut db)
.await?;
```

You can mix type-target and scoped forms in the same batch:

```rust,ignore
let (user, todo) = toasty::create!([
    User { name: "Carl" },
    in user.todos() { title: "Buy milk" },
])
.exec(&mut db)
.await?;
```

### Array and Vec of builders

When the number of records is determined at runtime, use an array or `Vec`
of create builders with `toasty::batch()`:

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
let users = toasty::batch([
    toasty::create!(User { name: "Alice", email: "alice@example.com" }),
    toasty::create!(User { name: "Bob", email: "bob@example.com" }),
    toasty::create!(User { name: "Carol", email: "carol@example.com" }),
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
    .map(|(i, n)| toasty::create!(User {
        name: *n,
        email: format!("user{i}@example.com"),
    }))
    .collect();

let users = toasty::batch(builders).exec(&mut db).await?;
```

## When to use the builder directly

The macro covers the common case. Use the builder directly when you need to
conditionally set fields, since the macro requires all fields to be specified
in the struct literal:

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
let mut builder = User::create().name("Alice");

if true /* some condition */ {
    builder = builder.bio("Likes Rust");
}

let user = builder.exec(&mut db).await?;
# Ok(())
# }
```

## Macro-to-builder reference

Each macro form has a direct builder equivalent:

| Macro syntax | Builder equivalent |
|---|---|
| `toasty::create!(User { name: "Alice" })` | `User::create().name("Alice")` |
| `toasty::create!(in user.todos() { title: "Buy milk" })` | `user.todos().create().title("Buy milk")` |
| Nested `{ ... }` for BelongsTo/HasOne | `.with_field(\|b\| b.field_calls)` |
| Nested `[{ ... }]` for HasMany | `.with_field(\|b\| b.with_item(...))` |

## What gets generated

For a model like `User`, `#[derive(Model)]` generates:

- `User::create()` — returns a builder with a setter for each non-auto field
- `toasty::create!(User { ... })` — macro syntax that expands to builder calls

The create builder's setter methods accept flexible input types through the
`IntoExpr` trait. For a `String` field, you can pass `&str`, `String`, or
`&String`. For numeric fields, you can pass the value directly or by reference.
See [Defining Models — What types can you pass to setters?](./defining-models.md#what-types-can-you-pass-to-setters)
for details.
