# Creating Records

Toasty provides the `toasty::create!` macro for inserting records. The macro
uses struct-literal syntax, so creating a record looks like constructing a
struct.

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

`toasty::create!` returns a create builder. Call `.exec(&mut db)` on it to
insert the row. The returned `User` instance has all fields set, including
auto-generated ones like `id`.

The generated SQL looks like:

```sql
INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');
```

Field values can be any Rust expression — literals, variables, or function
calls:

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

You don't need to set `user_id` — Toasty fills it in from the parent.

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

## How the macro maps to the builder

The `toasty::create!` macro is syntactic sugar over the generated create
builder API. Each form in the macro expands to builder method calls:

| Macro syntax | Builder equivalent |
|---|---|
| `toasty::create!(User { name: "Alice" })` | `User::create().name("Alice")` |
| `toasty::create!(in user.todos() { title: "Buy milk" })` | `user.todos().create().title("Buy milk")` |
| Nested `{ ... }` for BelongsTo/HasOne | `.with_field(|b| b.field_calls)` |
| Nested `[{ ... }]` for HasMany | `.with_field(|b| b.with_item(...))` |

For example, this macro call:

```rust,ignore
toasty::create!(User {
    name: "Alice",
    todos: [{ title: "Buy groceries" }, { title: "Write docs" }],
})
```

expands to:

```rust,ignore
User::create()
    .name("Alice")
    .with_todos(|b| {
        b.with_item(|b| b.title("Buy groceries"))
         .with_item(|b| b.title("Write docs"))
    })
```

## Using the builder directly

You can use the create builder without the macro. Call `Model::create()`,
chain setter methods for each field, and call `.exec(&mut db)`:

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

The builder is useful when you need to conditionally set fields:

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

The macro does not support conditional field assignment — use the builder
directly for that.

### Nested creation with the builder

Use the singular form of the relation name to add children:

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
let user = User::create()
    .name("Alice")
    .todo(Todo::create().title("Buy groceries"))
    .todo(Todo::create().title("Write docs"))
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

### Batch creation with the builder

Pass an array of create builders to `toasty::batch()`:

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
    User::create().name("Alice").email("alice@example.com"),
    User::create().name("Bob").email("bob@example.com"),
])
.exec(&mut db)
.await?;
# Ok(())
# }
```

Or use tuples for mixed models:

```rust,ignore
let (user, post): (User, Post) = toasty::batch((
    User::create().name("Alice"),
    Post::create().title("Hello World"),
))
.exec(&mut db)
.await?;
```

## What gets generated

For a model like `User`, `#[derive(Model)]` generates:

- `User::create()` — returns a builder with a setter for each non-auto field
- `toasty::create!(User { ... })` — macro syntax that expands to builder calls

The create builder's setter methods accept flexible input types through the
`IntoExpr` trait. For a `String` field, you can pass `&str`, `String`, or
`&String`. For numeric fields, you can pass the value directly or by reference.
See [Defining Models — What types can you pass to setters?](./defining-models.md#what-types-can-you-pass-to-setters)
for details.
