# Preloading Associations

Preloading (also called eager loading) loads related records alongside the main
query, avoiding extra database round-trips when you access associations.

## Async means a query, always

Toasty's API follows one rule for associations: **if you `.await` it, it hits
the database.** There are no hidden or implicit queries. The two ways to access
an association make this visible in the code:

- `user.posts().exec(&mut db).await?` — async, executes a query.
- `user.posts.get()` — not async, reads already-loaded data in memory.

Because `.get()` is a plain (non-async) method, the compiler won't let you
confuse the two. You can scan any code path for `.await` to know exactly where
database round-trips happen. This makes N+1 problems easy to spot and impossible
to introduce by accident.

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

Add `.include()` to a query to preload a relation. Pass the field path from the
model's `fields()` accessor:

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

## Preloaded vs unloaded access

There are two ways to access a relation, depending on whether it was preloaded:

| Access pattern | Async | When to use | Queries |
|---|---|---|---|
| `user.posts().exec(&mut db).await?` | Yes | Relation was not preloaded | Executes a query |
| `user.posts.get()` | No | Relation was preloaded with `.include()` | No query |

Because `.get()` is synchronous, you can never accidentally trigger a database
query by calling it. Conversely, every database round-trip requires `.await`, so
N+1 problems are always visible in the code.

Calling `.get()` on an unloaded relation panics. Only use `.get()` when you know
the relation was preloaded.

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
#     profile: toasty::HasOne<Option<Profile>>,
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
#     user: toasty::BelongsTo<Option<User>>,
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
#     profile: toasty::HasOne<Option<Profile>>,
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
#     user: toasty::BelongsTo<Option<User>>,
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
| `.include(Model::fields().relation())` | Preload a relation in the query |
| `model.relation.get()` | Access preloaded HasMany data (returns `&[T]`) |
| `model.relation.get()` | Access preloaded BelongsTo data (returns `&T`) |
| `model.relation.get()` | Access preloaded HasOne data (returns `&T` or `&Option<T>`) |
| `model.relation.is_unloaded()` | Check if a relation was preloaded |
