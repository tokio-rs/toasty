# HasMany

A `HasMany` relationship connects a parent model to multiple child records. The
parent declares a `Vec<T>` relation field, usually wrapped in `Deferred<_>` for
lazy loading. The child stores a foreign key pointing back to the parent via
[BelongsTo](./belongs-to.md).

## Defining a HasMany relationship

Add a `#[has_many]` field on the parent model. Use `Deferred<Vec<T>>` for lazy
loading, or `Vec<T>` for eager loading. The child model must have a
corresponding `#[belongs_to]` field.

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[has_many]
    posts: toasty::Deferred<Vec<Post>>,
}

#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: u64,

    #[index]
    user_id: u64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::Deferred<User>,

    title: String,
}
```

The `#[has_many]` attribute does not add any columns to the parent's table. The
relationship is stored entirely in the child's foreign key column (`user_id`).

With an eager field, Toasty loads the children whenever it loads the parent:

```rust,ignore
#[has_many]
posts: Vec<Post>,
```

This behaves like an implicit `.include(User::fields().posts())` on every query
that returns `User`.

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
#     posts: toasty::Deferred<Vec<Post>>,
# }
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     user_id: u64,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::Deferred<User>,
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
#     posts: toasty::Deferred<Vec<Post>>,
# }
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     user_id: u64,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::Deferred<User>,
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
#     posts: toasty::Deferred<Vec<Post>>,
# }
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     user_id: u64,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::Deferred<User>,
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

### Filtering with `.filter()`

```rust,ignore
// Find posts with a specific condition
let drafts = user
    .posts()
    .filter(Post::fields().published().eq(false))
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

## Multi-step relations (`via`)

A `HasMany` can reach its target through a path of existing relations instead of
a single foreign key. Declare it with `via` and a dotted chain of relation
fields, read left to right from this model. This expresses a relationship that
exists only *through* a third model — a user has many comments, each comment
belongs to an article, so a user has many commented articles:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[has_many]
    comments: toasty::Deferred<Vec<Comment>>,

    // User → comments → article
    #[has_many(via = comments.article)]
    commented_articles: toasty::Deferred<Vec<Article>>,
}

#[derive(Debug, toasty::Model)]
struct Comment {
    #[key]
    #[auto]
    id: u64,

    #[index]
    user_id: u64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::Deferred<User>,

    #[index]
    article_id: u64,

    #[belongs_to(key = article_id, references = id)]
    article: toasty::Deferred<Article>,
}

#[derive(Debug, toasty::Model)]
struct Article {
    #[key]
    #[auto]
    id: u64,

    title: String,

    #[has_many]
    comments: toasty::Deferred<Vec<Comment>>,
}
```

The target type is `Article` because the path `comments.article` ends there.
A `via` relation owns no foreign key — it is derived from the relations it
traverses — so it needs no `pair`. Each step may be any relation kind, and the
kinds can be mixed along one path.

Query a `via` relation like any other relation:

```rust,ignore
// Every article this user has commented on, each listed once.
let articles = user.commented_articles().exec(&mut db).await?;

// Filtered and ordered like any other relation query.
let recent = user
    .commented_articles()
    .filter(Article::fields().title().eq("Rust"))
    .exec(&mut db)
    .await?;
```

A `via` relation yields **distinct targets**: a target reached through several
intermediates appears once. A user who comments on the same article twice gets
that article once.

A `via` relation is **read-only**. You can query, filter, order, and preload it,
but Toasty generates no `create`, `insert`, or `remove` methods — writing
through a multi-step path would have to materialize intermediate records. Mutate
the underlying relations directly instead.

Preload a `via` relation with `.include()` to avoid the N+1 — see
[Preloading Associations](./preloading-associations.md). Preloading or
projecting a `via` relation is supported on SQL backends; both are not yet
available on DynamoDB.

## What gets generated

For a `User` model with `#[has_many] posts: Deferred<Vec<Post>>`, Toasty generates:

**On the parent instance:**

| Method | Returns | Description |
|---|---|---|
| `user.posts()` | Relation accessor | Accessor scoped to this user's posts |
| `.exec(&mut db)` | `Result<Vec<Post>>` | All posts belonging to this user |
| `.create()` | Create builder | Create a post with the foreign key pre-filled |
| `.get_by_id(&mut db, &id)` | `Result<Post>` | Get a post by ID within the scope |
| `.filter(expr)` | Query builder | Filter posts within the scope |
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
