# Keys and Auto-Generation

Every model needs a primary key. Toasty uses the `#[key]` attribute to mark
which field (or fields) form the primary key, and `#[auto]` to auto-generate
values for key fields.

## Single-field keys

Mark a field with `#[key]` to make it the primary key:

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,
}
```

This generates `User::get_by_id()` to fetch a user by primary key:

```rust,ignore
let user = User::get_by_id(&mut db, &some_id).await?;
```

## Auto-generated values

The `#[auto]` attribute tells Toasty to generate the field's value
automatically. You don't set auto fields when creating a record — Toasty fills
them in.

```rust,ignore
// No need to set `id` — it's auto-generated
let user = User::create()
    .name("Alice")
    .exec(&mut db)
    .await?;

// The generated id is available on the returned value
println!("id: {}", user.id);
```

### Auto strategies

The behavior of `#[auto]` depends on the field type:

| Field type | `#[auto]` behavior | Explicit form |
|---|---|---|
| `uuid::Uuid` | Generates a UUID v7 | `#[auto(uuid(v7))]` |
| `i64`, `i32`, etc. | Auto-incrementing integer | `#[auto(increment)]` |

You can specify the strategy explicitly:

```rust,ignore
// UUID v7 (time-ordered, the default for Uuid)
#[auto(uuid(v7))]
id: uuid::Uuid,

// UUID v4 (random)
#[auto(uuid(v4))]
id: uuid::Uuid,

// Auto-incrementing integer
#[auto(increment)]
id: i64,
```

### UUID v7 vs v4

UUID v7 values are time-ordered — UUIDs created later sort after earlier ones.
This is the default for `uuid::Uuid` fields because time-ordered keys perform
better in database indexes.

UUID v4 values are random with no ordering.

### Integer auto-increment

Integer keys use the database's auto-increment feature:

```rust
#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto(increment)]
    id: i64,

    title: String,
}
```

```rust,ignore
let post = Post::create()
    .title("Hello World")
    .exec(&mut db)
    .await?;

println!("post id: {}", post.id); // 1, 2, 3, ...
```

## Composite keys

For databases like DynamoDB that use partition and sort keys, Toasty supports
composite keys with the `#[key]` attribute on the struct:

```rust
#[derive(Debug, toasty::Model)]
#[key(partition = user_id, local = id)]
struct Todo {
    #[auto]
    id: uuid::Uuid,

    title: String,

    user_id: uuid::Uuid,
}
```

The `partition` field determines which partition the record is stored in. The
`local` field uniquely identifies the record within that partition.

With composite keys, Toasty generates lookup methods that take both fields:

```rust,ignore
let todo = Todo::get_by_user_id_and_id(
    &mut db, &user_id, &todo_id
).await?;
```

You can also query by just the partition key to get all records in a partition:

```rust,ignore
let todos = Todo::filter_by_user_id(&user_id)
    .all(&mut db)
    .await?;
```

## What gets generated

For a model with `#[key]`, Toasty generates these methods:

| Attribute | Generated methods |
|---|---|
| `#[key]` on single field | `get_by_<field>()`, `filter_by_<field>()`, `delete_by_<field>()` |
| `#[key(partition = a, local = b)]` | `get_by_<a>_and_<b>()`, `filter_by_<a>()`, `filter_by_<a>_and_<b>()`, `delete_by_<a>_and_<b>()` |
