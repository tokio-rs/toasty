# Batch Operations

`toasty::stmt::batch()` executes multiple queries or creates in a single database
round-trip. Instead of sending each query separately, Toasty combines them into
one composed statement.

Batch operations are **atomic**, database permitting — all operations in a batch
either succeed together or fail together. When you need atomicity, prefer batch
operations over
[interactive transactions](./transactions.md). Batch operations are more
efficient because they can be sent as a single statement to the database, while
interactive transactions require separate round-trips to begin the transaction,
execute each statement, and commit. In many cases, batch operations are
sufficient. Reach for interactive transactions only when you need to read data
and make decisions based on those reads within the same atomic scope.

## Batching queries with tuples

Pass a tuple of queries to `toasty::stmt::batch()`. The return type matches the tuple
structure:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     name: String,
# }
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     title: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let (users, posts): (Vec<User>, Vec<Post>) = toasty::stmt::batch((
    User::filter_by_name("Alice"),
    Post::filter_by_title("Hello"),
))
.exec(&mut db)
.await?;
# Ok(())
# }
```

Each element in the tuple is an independent query. Toasty sends them together
and returns the results in the same tuple order. Tuples support up to 8
elements.

You can batch queries for the same model too:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     name: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let (alices, bobs): (Vec<User>, Vec<User>) = toasty::stmt::batch((
    User::filter_by_name("Alice"),
    User::filter_by_name("Bob"),
))
.exec(&mut db)
.await?;
# Ok(())
# }
```

## Batching with arrays and Vecs

When all queries are the same type, use an array or `Vec` instead of a tuple.
The return type is `Vec<Vec<Model>>` — one inner `Vec` per query:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     name: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let results: Vec<Vec<User>> = toasty::stmt::batch([
    User::filter_by_name("Alice"),
    User::filter_by_name("Bob"),
    User::filter_by_name("Carol"),
])
.exec(&mut db)
.await?;

assert_eq!(results.len(), 3); // one result set per query
# Ok(())
# }
```

This works with `Vec` too, which is useful when the number of queries is
determined at runtime:

```rust,ignore
let names = vec!["Alice", "Bob", "Carol"];
let queries: Vec<_> = names
    .iter()
    .map(|n| User::filter_by_name(*n))
    .collect();

let results: Vec<Vec<User>> = toasty::stmt::batch(queries)
    .exec(&mut db)
    .await?;
```

## Batching creates

`toasty::stmt::batch()` also accepts create builders. Mix creates and queries in the
same batch using tuples:

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
#     title: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let (user, post): (User, Post) = toasty::stmt::batch((
    User::create().name("Alice"),
    Post::create().title("Hello World"),
))
.exec(&mut db)
.await?;
# Ok(())
# }
```

Each create returns a single record (not a `Vec`), since each create builder
inserts exactly one row.

## Bulk creation with `create_many()`

`Model::create_many()` inserts multiple records of the same model. Add records
with `.item()` or `.with_item()`:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Todo {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let todos = Todo::create_many()
    .item(Todo::create().title("Buy groceries"))
    .item(Todo::create().title("Write docs"))
    .item(Todo::create().title("Ship feature"))
    .exec(&mut db)
    .await?;

assert_eq!(todos.len(), 3);
# Ok(())
# }
```

`.item()` takes a create builder. `.with_item()` takes a closure that receives
the create builder, which is useful for inline construction:

```rust,ignore
let todos = Todo::create_many()
    .with_item(|c| c.title("Buy groceries"))
    .with_item(|c| c.title("Write docs"))
    .exec(&mut db)
    .await?;
```

`create_many()` returns a `Vec` of the created records, including auto-generated
fields like `id`.

## `create_many()` vs `batch()` for inserts

Both can insert multiple records, but they differ:

| | `create_many()` | `batch()` |
|---|---|---|
| **Scope** | Single model | Any mix of models, queries, and creates |
| **Return type** | `Vec<Model>` | Matches the input structure |
| **Use case** | Insert many records of the same type | Combine diverse operations |

Use `create_many()` when inserting multiple records of the same model. Use
`batch()` when combining different operations or models.

## Batch key lookups

For primary key fields, Toasty generates a `filter_by_<key>_batch()` method that
fetches multiple records by key in a single query. This translates to an SQL
`IN` clause:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let users = User::filter_by_id_batch([&1, &2, &3])
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

This is more efficient than calling `get_by_id()` in a loop. See
[Querying Records](./querying-records.md#batch-key-lookups) for more details.
