# Querying Records

Toasty generates several ways to retrieve records: by primary key, by indexed
fields, or by building queries with filters.

## Get by primary key

`Model::get_by_id()` fetches a single record by its primary key. It returns the
record directly, or an error if no record exists with that key.

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
let user = User::get_by_id(&mut db, &1).await?;
println!("Found: {}", user.name);
# Ok(())
# }
```

The method name matches the key field name. A model with `#[key] code: String`
generates `get_by_code()`. Composite keys generate combined names like
`get_by_student_id_and_course_id()`.

## Get all records

`Model::all()` returns a query for all records of that model.

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
let users = User::all().collect::<Vec<_>>(&mut db).await?;

for user in &users {
    println!("{}: {}", user.id, user.name);
}
# Ok(())
# }
```

## Executing queries

Queries returned by `all()`, `filter()`, and `filter_by_*()` are not executed
until you call a terminal method. Toasty provides four terminal methods:

### `.all()` — collect all results

Returns all matching records as a `Vec`:

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
let users: Vec<User> = User::all().all(&mut db).await?;
# Ok(())
# }
```

### `.collect::<Vec<_>>()` — collect into a collection

Collects results into any type that implements `Extend` and `Default`. In
practice, this means `Vec<Model>`:

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
let users: Vec<User> = User::all().collect::<Vec<_>>(&mut db).await?;
# Ok(())
# }
```

### `.first()` — get the first result or `None`

Returns `Option<Model>` — `Some` if at least one record matches, `None` if the
query returns no results:

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
let maybe_user = User::all().first(&mut db).await?;

match maybe_user {
    Some(user) => println!("Found: {}", user.name),
    None => println!("No users found"),
}
# Ok(())
# }
```

### `.get()` — get exactly one result

Returns the record directly, or an error if no record matches. Use this when you
expect the query to return exactly one result:

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
# async fn __example(mut db: toasty::Db, user_id: u64) -> toasty::Result<()> {
let user = User::filter_by_id(user_id).get(&mut db).await?;
# Ok(())
# }
```

## Filtering by indexed fields

Toasty generates `filter_by_*` methods for indexed and key fields. These return
a query builder that you can execute with any terminal method.

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
// filter_by_id returns a query builder
let user = User::filter_by_id(1).get(&mut db).await?;

// filter_by_email is generated because email has #[unique]
let user = User::filter_by_email("alice@example.com")
    .get(&mut db)
    .await?;
# Ok(())
# }
```

The difference between `get_by_*` and `filter_by_*`: `get_by_*` methods execute
immediately and return the record. `filter_by_*` methods return a query builder
that you can further customize before executing.

## Filtering with expressions

For queries beyond simple field equality, use `Model::filter()` with field
expressions. The [Filtering with Expressions](./filtering-with-expressions.md)
chapter covers this in detail. Here is a quick example:

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
let users = User::filter(User::fields().name().eq("Alice"))
    .all(&mut db)
    .await?;
# Ok(())
# }
```

## Chaining filters

You can chain `.filter()` on an existing query to add more conditions:

```rust,ignore
let users = User::filter_by_name("Alice")
    .filter(User::fields().age().gt(25))
    .all(&mut db)
    .await?;
```

Each `.filter()` call adds an AND condition to the query.

## Batch key lookups

For primary key fields, Toasty generates a `filter_by_<key>_batch()` method that
fetches multiple records by key in a single query:

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
let users = User::filter_by_id_batch([&1, &2, &3])
    .collect::<Vec<_>>(&mut db)
    .await?;
# Ok(())
# }
```

This is more efficient than calling `get_by_id()` in a loop.

## What gets generated

For a model with `#[key]` on `id` and `#[unique]` on `email`, Toasty generates:

| Method | Returns | Description |
|---|---|---|
| `User::all()` | Query builder | All records |
| `User::filter(expr)` | Query builder | Records matching expression |
| `User::filter_by_id(id)` | Query builder | Records matching key |
| `User::filter_by_id_batch(ids)` | Query builder | Records matching any key in list |
| `User::filter_by_email(email)` | Query builder | Records matching unique field |
| `User::get_by_id(&mut db, &id)` | `Result<User>` | One record by key (immediate) |
| `User::get_by_email(&mut db, email)` | `Result<User>` | One record by unique field (immediate) |

Query builders support these terminal methods:

| Method | Returns |
|---|---|
| `.all(&mut db)` | `Result<Vec<User>>` |
| `.collect::<Vec<_>>(&mut db)` | `Result<Vec<User>>` |
| `.first(&mut db)` | `Result<Option<User>>` |
| `.get(&mut db)` | `Result<User>` |
