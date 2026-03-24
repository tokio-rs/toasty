# HasMany

A `HasMany` relationship connects a parent model to multiple child records. The
parent declares a `HasMany<T>` field, and the child stores a foreign key
pointing back to the parent via [BelongsTo](./belongs-to.md).

## Defining a HasMany relationship

Add a `#[has_many]` field of type `HasMany<T>` on the parent model. The child
model must have a corresponding `#[belongs_to]` field.

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

The `#[has_many]` attribute does not add any columns to the parent's table. The
relationship is stored entirely in the child's foreign key column (`user_id`).

## Querying children

Call the relation method on a parent instance to get an accessor for its
children:

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
let posts: Vec<Post> = user.posts().exec(&mut db).await?;

for post in &posts {
    println!("{}", post.title);
}
# Ok(())
# }
```

The generated SQL is:

```sql
SELECT * FROM posts WHERE user_id = ?;
```

All queries through the relation accessor are automatically scoped to the
parent. `user.posts()` only returns posts belonging to that user.

## Creating through the relation

Create a child record through the parent's relation accessor. Toasty
automatically sets the foreign key:

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
let post = toasty::create!(in user.posts() { title: "Hello World" })
    .exec(&mut db)
    .await?;

assert_eq!(post.user_id, user.id);
# Ok(())
# }
```

You don't need to set `user_id` — Toasty fills it in from the parent.

## Nested creation

Create a parent and its children in a single call using the singular form of the
relation name on the create builder:

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
let user = toasty::create!(User {
    name: "Alice",
    posts: [{ title: "First post" }, { title: "Second post" }],
})
.exec(&mut db)
.await?;

let posts = user.posts().exec(&mut db).await?;
assert_eq!(2, posts.len());
# Ok(())
# }
```

Toasty creates the user first, then creates each post with the user's ID as the
foreign key.

## Inserting and removing children

Use `.insert()` and `.remove()` to link and unlink existing records.

### Inserting

Associate an existing child record with a parent:

```rust,ignore
let post = toasty::create!(Post { title: "Orphan post", user_id: 0 }).exec(&mut db).await?;

// Associate the post with a user
user.posts().insert(&mut db, &post).await?;
```

This updates the child's foreign key to point to the parent. You can also insert
multiple records at once:

```rust,ignore
user.posts().insert(&mut db, &[post1, post2, post3]).await?;
```

If the child is already associated with a different parent, `.insert()` moves it
to the new parent.

### Removing

Disassociate a child from a parent:

```rust,ignore
user.posts().remove(&mut db, &post).await?;
```

What happens to the child depends on whether the foreign key is required or
optional:

| Foreign key type | Effect of `.remove()` |
|---|---|
| Required (`user_id: u64`) | Deletes the child record |
| Optional (`user_id: Option<u64>`) | Sets the foreign key to `NULL` |

When the foreign key is required, the child cannot exist without a parent, so
Toasty deletes it. When the foreign key is optional, the child remains in the
database with a null foreign key.

## Scoped queries

The relation accessor supports scoped queries — filtering, updating, and
deleting within the parent's children.

### Filtering with `.query()`

```rust,ignore
// Find posts with a specific condition
let drafts = user
    .posts()
    .query(Post::fields().published().eq(false))
    .exec(&mut db)
    .await?;
```

### Looking up by ID within the scope

```rust,ignore
let post = user.posts().get_by_id(&mut db, &post_id).await?;
```

This only returns the post if it belongs to the user. If the post exists but
belongs to a different user, this returns an error.

### Updating through the scope

```rust,ignore
user.posts()
    .filter_by_id(post_id)
    .update()
    .title("New title")
    .exec(&mut db)
    .await?;
```

### Deleting through the scope

```rust,ignore
user.posts()
    .filter_by_id(post_id)
    .delete()
    .exec(&mut db)
    .await?;
```

## Filtering parents by children

You can filter parent records based on conditions on their children using
`.any()` on the relation field:

```rust,ignore
// Find users who have at least one published post
let users = User::filter(
    User::fields()
        .posts()
        .any(Post::fields().published().eq(true))
)
.exec(&mut db)
.await?;
```

## What gets generated

For a `User` model with `#[has_many] posts: HasMany<Post>`, Toasty generates:

**On the parent instance:**

| Method | Returns | Description |
|---|---|---|
| `user.posts()` | Relation accessor | Accessor scoped to this user's posts |
| `.exec(&mut db)` | `Result<Vec<Post>>` | All posts belonging to this user |
| `.create()` | Create builder | Create a post with the foreign key pre-filled |
| `.get_by_id(&mut db, &id)` | `Result<Post>` | Get a post by ID within the scope |
| `.query(expr)` | Query builder | Filter posts within the scope |
| `.insert(&mut db, &post)` | `Result<()>` | Associate an existing post with the user |
| `.remove(&mut db, &post)` | `Result<()>` | Disassociate a post from the user |

**On the create builder:**

| Method | Description |
|---|---|
| `toasty::create!(User { posts: [{ ... }] })` | Add children to create alongside the parent |

**On the fields accessor:**

| Method | Description |
|---|---|
| `User::fields().posts()` | Field path for preloading and filtering |
| `User::fields().posts().any(expr)` | Filter parents by child conditions |
