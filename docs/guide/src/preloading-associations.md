# Preloading Associations

Preloading (also called eager loading) loads related records alongside the main
query, avoiding extra database round-trips when you access associations.

## Async means a query

Toasty's API follows one rule for associations: **if you `.await` it, it hits
the database.** Lazy relation fields make this visible in the code:

- `user.posts().exec(&mut db).await?` — async, executes a query.
- `user.posts.get()` — not async, reads already-loaded data in memory.

Because `.get()` is a plain (non-async) method, the compiler won't let you
confuse the two. Plain eager relation fields are already-loaded values, so
accessing `user.posts` also does not issue a query. You can scan any code path
for `.await` to know exactly where database round-trips happen. This makes N+1
problems easy to spot and impossible to introduce by accident.

## The N+1 problem

Without preloading, accessing a relation on each record in a list causes one
query per record:

```rust,ignore
// 1 query: load all users
let users = User::all().exec(&mut db).await?;

for user in &users {
    // N queries: one per user to load their posts
    let posts = user.posts().exec(&mut db).await?;
    println!("{}: {} posts", user.name, posts.len());
}
```

If there are 100 users, this executes 101 queries. The `.await` on each
`user.posts().exec()` call is a clear signal that a query runs on every
iteration. Preloading reduces this to a fixed number of queries regardless of
how many records you load.

## Using `.include()`

Add `.include()` to a query to preload a `Deferred<_>` relation. Pass the field
path from the model's `fields()` accessor:

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
# let user = toasty::create!(User { name: "Alice", posts: [{ title: "Hello" }] })
#     .exec(&mut db)
#     .await?;
let user = User::filter_by_id(user.id)
    .include(User::fields().posts())
    .get(&mut db)
    .await?;

// Access preloaded posts — .get() is not async, so no query happens
let posts: &[Post] = user.posts.get();
assert_eq!(1, posts.len());
# Ok(())
# }
```

The `.include()` call tells Toasty to load the associated posts as part of the
query. After preloading, access the data through the field directly with
`user.posts.get()` — a synchronous call that reads from memory. Compare this
with `user.posts().exec(&mut db).await?`, which is async and always runs a
query. The presence or absence of `.await` tells you whether code touches the
database.

## Eager relation fields

A relation field that is not wrapped in `Deferred<_>` is loaded by every query
that returns the model. This is equivalent to an implicit `.include(...)`.

```rust,ignore
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    #[has_many]
    posts: Vec<Post>,
}

let user = User::filter_by_id(user_id).get(&mut db).await?;

// Already loaded — no .include() and no .get() wrapper.
let post_count = user.posts.len();
```

Use eager relation fields when most queries for the model need the relation. Use
`Deferred<_>` when callers should choose between lazy access and explicit
`.include(...)`.

Eager relations cannot form cycles. Toasty rejects a schema where eager loading
would recurse forever, such as `User.posts: Vec<Post>` paired with
`Post.user: User`. Make one side `Deferred<_>` when two models point at each
other.

## Preloaded vs unloaded access

There are three access patterns for relations:

| Access pattern | Async | When to use | Queries |
|---|---|---|---|
| `user.posts().exec(&mut db).await?` | Yes | Lazy relation was not preloaded | Executes a query |
| `user.posts.get()` | No | `Deferred<_>` relation was preloaded with `.include()` | No query |
| `user.posts` | No | Relation field is not wrapped in `Deferred<_>` | No query |

Calling `.get()` on an unloaded relation panics. Only use `.get()` when you know
the relation was preloaded.

### `.try_get()` when the load state is uncertain

When a function receives a record from a caller it does not control, it may not
know whether a given relation was preloaded. Use `.try_get()` to access the
loaded value without panicking — it returns `None` if the relation has not been
loaded:

```rust,ignore
fn post_count(user: &User) -> Option<usize> {
    user.posts.try_get().map(<[_]>::len)
}
```

`.try_get()` is available on relation fields and returns the same reference
shape as `.get()` wrapped in an `Option`:

| Field type | `.get()` returns | `.try_get()` returns |
|---|---|---|
| `Deferred<T>` | `&T` | `Option<&T>` |
| `Deferred<Option<T>>` | `&Option<T>` | `Option<&Option<T>>` |
| `Deferred<Vec<T>>` | `&Vec<T>` | `Option<&Vec<T>>` |

For `Deferred<Vec<T>>`, an empty vector means the association was loaded and
the record has no related rows, while `None` from `.try_get()` means the
association was not loaded.

Prefer `.get()` in code paths that control the query (the call site can see the
matching `.include()`); reserve `.try_get()` for code that accepts records from
elsewhere and needs to fall back when the data is missing.

## Preloading BelongsTo

Preload a parent record from the child side:

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
# let user = toasty::create!(User { name: "Alice", posts: [{ title: "Hello" }] })
#     .exec(&mut db)
#     .await?;
# let post_id = user.posts().exec(&mut db).await?[0].id;
let post = Post::filter_by_id(post_id)
    .include(Post::fields().user())
    .get(&mut db)
    .await?;

// Access the preloaded user
let user: &User = post.user.get();
assert_eq!("Alice", user.name);
# Ok(())
# }
```

## Preloading HasOne

Preload a single child record from the parent side:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_one]
#     profile: toasty::Deferred<Option<Profile>>,
# }
# #[derive(Debug, toasty::Model)]
# struct Profile {
#     #[key]
#     #[auto]
#     id: u64,
#     bio: String,
#     #[unique]
#     user_id: Option<u64>,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::Deferred<Option<User>>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let user = toasty::create!(User { name: "Alice", profile: { bio: "A person" } })
#     .exec(&mut db)
#     .await?;
let user = User::filter_by_id(user.id)
    .include(User::fields().profile())
    .get(&mut db)
    .await?;

// Access the preloaded profile
let profile = user.profile.get().as_ref().unwrap();
assert_eq!("A person", profile.bio);
# Ok(())
# }
```

If no related record exists, the preloaded value is `None` rather than causing a
panic:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_one]
#     profile: toasty::Deferred<Option<Profile>>,
# }
# #[derive(Debug, toasty::Model)]
# struct Profile {
#     #[key]
#     #[auto]
#     id: u64,
#     bio: String,
#     #[unique]
#     user_id: Option<u64>,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::Deferred<Option<User>>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let user = toasty::create!(User { name: "No Profile" }).exec(&mut db).await?;

let user = User::filter_by_id(user.id)
    .include(User::fields().profile())
    .get(&mut db)
    .await?;

// Preloaded and empty — .get() returns None, not a panic
assert!(user.profile.get().is_none());
# Ok(())
# }
```

## Preloading multi-step (`via`) relations

A [multi-step (`via`) relation](./has-many.md#multi-step-relations-via) reaches
its target through a path of existing relations. Preload it with `.include()`
the same way as a single-step relation:

```rust,ignore
let users = User::all()
    .include(User::fields().commented_articles())
    .exec(&mut db)
    .await?;

for user in &users {
    // Distinct articles this user has commented on — no N+1.
    let articles: &[Article] = user.commented_articles.get();
}
```

Toasty loads the parents once, then issues a single query that follows the
relation path and groups the targets under each parent. Duplicate targets are
collapsed, so an article a user commented on twice appears once. A
`has_one(via = ...)` relation preloads a single optional target instead of a
list.

Preloading a `via` relation with `.include()` (and projecting one with
`.select()`) is supported on SQL backends (SQLite, PostgreSQL, MySQL). It is
not yet available on DynamoDB.

## Multiple includes

Chain multiple `.include()` calls to preload several relations in one query:

```rust,ignore
let user = User::filter_by_id(user_id)
    .include(User::fields().posts())
    .include(User::fields().comments())
    .get(&mut db)
    .await?;

// Both are preloaded
let posts: &[Post] = user.posts.get();
let comments: &[Comment] = user.comments.get();
```

You can mix relation types — preload HasMany, HasOne, and BelongsTo relations in
the same query:

```rust,ignore
let user = User::filter_by_id(user_id)
    .include(User::fields().profile())   // HasOne
    .include(User::fields().posts())     // HasMany
    .get(&mut db)
    .await?;
```

## Preloading with collection queries

`.include()` works with `.exec()` (collection queries), not just `.get()`
(single record). All records in the result have their relations preloaded:

```rust,ignore
let users = User::all()
    .include(User::fields().posts())
    .exec(&mut db)
    .await?;

for user in &users {
    // .get() is not async — no query per user, no N+1
    let posts: &[Post] = user.posts.get();
    println!("{}: {} posts", user.name, posts.len());
}
```

## Summary

| Syntax | Description |
|---|---|
| `.include(Model::fields().relation())` | Preload a `Deferred<_>` relation in the query |
| `model.relation.get()` | Access preloaded `Deferred<Vec<T>>`, `Deferred<T>`, or `Deferred<Option<T>>` data |
| `model.relation` | Access an eager `Vec<T>`, `T`, or `Option<T>` relation field |
| `model.relation.try_get()` | Non-panicking access; returns `None` if not preloaded |
| `model.relation.is_unloaded()` | Check whether a `Deferred<_>` relation was preloaded |
