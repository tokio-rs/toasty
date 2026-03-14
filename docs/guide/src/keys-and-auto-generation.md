# Keys and Auto-Generation

Every model needs a primary key. Toasty uses the `#[key]` attribute to mark
which field (or fields) form the primary key, and `#[auto]` to optionally
auto-generate values for key fields.

## Single-field keys

Mark a field with `#[key]` to make it the primary key:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,
}
```

This generates `User::get_by_id()` to fetch a user by primary key:

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
let user = User::get_by_id(&mut db, &1).await?;
# Ok(())
# }
```

### Keys without `#[auto]`

The `#[auto]` attribute is optional. Without it, you are responsible for
providing the key value when creating a record and for ensuring uniqueness:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct Country {
    #[key]
    code: String,

    name: String,
}
```

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Country {
#     #[key]
#     code: String,
#     name: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let country = Country::create()
    .code("US")
    .name("United States")
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

### Other key types

The primary key field can be any supported type. UUID is a common choice:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,
}
```

When `#[auto]` is used on a `uuid::Uuid` field, Toasty generates a UUID v7
(time-ordered) by default. See [auto strategies](#auto-strategies) for other
options.

## Auto-generated values

The `#[auto]` attribute tells Toasty to generate the field's value
automatically. You don't set auto fields when creating a record — Toasty fills
them in.

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
// No need to set `id` — it's auto-generated
let user = User::create()
    .name("Alice")
    .exec(&mut db)
    .await?;

// The generated id is available on the returned value
println!("id: {}", user.id);
# Ok(())
# }
```

### Auto strategies

The behavior of `#[auto]` depends on the field type:

| Field type | `#[auto]` behavior | Explicit form |
|---|---|---|
| `uuid::Uuid` | Generates a UUID v7 | `#[auto(uuid(v7))]` |
| `u64`, `i64`, etc. | Auto-incrementing integer | `#[auto(increment)]` |

You can specify the strategy explicitly:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct ExampleA {
#     #[key]
// UUID v7 (time-ordered, the default for Uuid)
#[auto(uuid(v7))]
id: uuid::Uuid,
#     name: String,
# }

# #[derive(Debug, toasty::Model)]
# struct ExampleB {
#     #[key]
// UUID v4 (random)
#[auto(uuid(v4))]
id: uuid::Uuid,
#     name: String,
# }

# #[derive(Debug, toasty::Model)]
# struct ExampleC {
#     #[key]
// Auto-incrementing integer
#[auto(increment)]
id: i64,
#     name: String,
# }
```

### UUID v7 vs v4

UUID v7 values are time-ordered — UUIDs created later sort after earlier ones.
This is the default for `uuid::Uuid` fields because time-ordered keys perform
better in database indexes.

UUID v4 values are random with no ordering.

### Integer auto-increment

Integer keys use the database's auto-increment feature:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto(increment)]
    id: i64,

    title: String,
}
```

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto(increment)]
#     id: i64,
#     title: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let post = Post::create()
    .title("Hello World")
    .exec(&mut db)
    .await?;

println!("post id: {}", post.id); // 1, 2, 3, ...
# Ok(())
# }
```

Auto-increment requires database support. SQLite, PostgreSQL, and MySQL all
support auto-incrementing columns. DynamoDB does not.

## Composite keys

A composite key uses two or more fields as the primary key. Toasty supports two
ways to define composite keys.

### Multiple `#[key]` fields

Mark each key field with `#[key]`:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct Enrollment {
    #[key]
    student_id: u64,

    #[key]
    course_id: u64,

    grade: Option<String>,
}
```

This generates lookup methods that take both fields:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Enrollment {
#     #[key]
#     student_id: u64,
#     #[key]
#     course_id: u64,
#     grade: Option<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let enrollment = Enrollment::get_by_student_id_and_course_id(
    &mut db, &1, &101
).await?;
# Ok(())
# }
```

### Partition and local keys

For databases like DynamoDB that use partition and sort keys, use the
`#[key(partition = ..., local = ...)]` attribute on the struct:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
#[key(partition = user_id, local = id)]
struct Todo {
    #[auto]
    id: u64,

    title: String,

    user_id: u64,
}
```

The `partition` field determines which partition the record is stored in. The
`local` field uniquely identifies the record within that partition.

With partition/local keys, Toasty generates methods to query by both fields or
by the partition key alone:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# #[key(partition = user_id, local = id)]
# struct Todo {
#     #[auto]
#     id: u64,
#     title: String,
#     user_id: u64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Get a specific todo
let todo = Todo::get_by_user_id_and_id(
    &mut db, &1, &42
).await?;

// Get all todos for a user
let todos = Todo::filter_by_user_id(&1)
    .all(&mut db)
    .await?;
# Ok(())
# }
```

## What gets generated

For a model with `#[key]`, Toasty generates these methods:

| Attribute | Generated methods |
|---|---|
| `#[key]` on single field | `get_by_<field>()`, `filter_by_<field>()`, `delete_by_<field>()` |
| `#[key]` on multiple fields | `get_by_<a>_and_<b>()`, `filter_by_<a>_and_<b>()`, `delete_by_<a>_and_<b>()` |
| `#[key(partition = a, local = b)]` | `get_by_<a>_and_<b>()`, `filter_by_<a>()`, `filter_by_<a>_and_<b>()`, `delete_by_<a>_and_<b>()` |
