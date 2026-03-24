# BelongsTo

A `BelongsTo` relationship connects a child model to a parent model through a
foreign key. The child stores the parent's ID in one of its own fields.

## Defining a BelongsTo relationship

A BelongsTo relationship requires two things on the child model: a foreign key
field and a `BelongsTo<T>` relation field. The `#[belongs_to]` attribute tells
Toasty which field holds the foreign key and which field on the parent it
references.

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[has_many]
    posts: toasty::HasMany<Post>,
}

#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: u64,

    #[index]
    user_id: u64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,

    title: String,
}
```

The `user_id` field is the foreign key — it stores the `id` of the associated
`User`. The `#[belongs_to(key = user_id, references = id)]` attribute tells
Toasty that `user_id` on `Post` maps to `id` on `User`.

The foreign key field should have `#[index]` so that Toasty can efficiently look
up posts by user. In the database, this creates:

```sql
CREATE TABLE posts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    title TEXT NOT NULL
);
CREATE INDEX idx_posts_user_id ON posts (user_id);
```

The parent model (`User`) typically declares a `#[has_many]` field pointing back
at the child. See [HasMany](./has-many.md) for details.

## Optional BelongsTo

If a child does not always have a parent, make the foreign key `Option<T>` and
wrap the relation type in `Option`:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
# }
#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: u64,

    #[index]
    user_id: Option<u64>,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<Option<User>>,

    title: String,
}
```

The `user_id` column is now nullable. A post can exist without a user.

## Accessing the related record

Call the relation method on the child instance to get the parent. The method
name matches the relation field name.

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_many]
#     posts: toasty::HasMany<Post>,
# }
# #[derive(Debug, toasty::Model)]
# struct Post {
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
# let post = toasty::create!(Post { title: "Hello", user_id: 1 }).exec(&mut db).await?;
// Load the associated user from the database
let user = post.user().exec(&mut db).await?;
println!("Author: {}", user.name);
# Ok(())
# }
```

For an optional BelongsTo, `.get()` returns `Option<User>`:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
# }
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     user_id: Option<u64>,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::BelongsTo<Option<User>>,
#     title: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let post = toasty::create!(Post { title: "Hello" }).exec(&mut db).await?;
match post.user().exec(&mut db).await? {
    Some(user) => println!("Author: {}", user.name),
    None => println!("No author"),
}
# Ok(())
# }
```

Each call to `.user().exec()` executes a database query. To avoid repeated
queries, use [preloading](./preloading-associations.md).

## Setting the relation on create

You can associate a child with a parent in two ways: by passing a reference to
the parent, or by setting the foreign key directly.

### By parent reference

Pass a reference to an existing parent record:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_many]
#     posts: toasty::HasMany<Post>,
# }
# #[derive(Debug, toasty::Model)]
# struct Post {
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
let user = toasty::create!(User { name: "Alice" }).exec(&mut db).await?;

let post = toasty::create!(Post {
    title: "Hello World",
    user: &user,
})
.exec(&mut db)
.await?;

assert_eq!(post.user_id, user.id);
# Ok(())
# }
```

Toasty extracts the parent's primary key and sets the foreign key field
automatically.

### By foreign key value

Set the foreign key field directly:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_many]
#     posts: toasty::HasMany<Post>,
# }
# #[derive(Debug, toasty::Model)]
# struct Post {
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
let post = toasty::create!(Post {
    title: "Hello World",
    user_id: user.id,
})
.exec(&mut db)
.await?;
# Ok(())
# }
```

This is useful when you have the parent's ID but not the full record.

## Customizing the pair name

By default, Toasty matches a `#[has_many]` field on the parent to a
`#[belongs_to]` field on the child by the singularized parent model name. If the
child's relation field has a different name, use `#[has_many(pair = field_name)]`
on the parent:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    #[has_many(pair = owner)]
    todos: toasty::HasMany<Todo>,
}

#[derive(Debug, toasty::Model)]
struct Todo {
    #[key]
    #[auto]
    id: u64,

    #[index]
    owner_id: u64,

    #[belongs_to(key = owner_id, references = id)]
    owner: toasty::BelongsTo<User>,

    title: String,
}
```

Here the child's relation field is named `owner` instead of `user`, so the
parent specifies `pair = owner` to establish the connection.

## What gets generated

For a `Post` model with `#[belongs_to] user: BelongsTo<User>`, Toasty generates:

| Method | Returns | Description |
|---|---|---|
| `post.user()` | Relation accessor | Returns an accessor for the associated user |
| `.get(&mut db)` | `Result<User>` | Loads the associated user from the database |
| `toasty::create!(Post { user: &user })` | Create builder | Sets the foreign key from a parent reference |
| `toasty::create!(Post { user_id: id })` | Create builder | Sets the foreign key directly |
| `Post::fields().user()` | Field path | Used with `.include()` for preloading |
