# Defining Models

A model is a Rust struct annotated with `#[derive(toasty::Model)]`. Each struct
maps to a database table and each field maps to a column.

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,
    email: String,
}
```

This creates a `users` table with three columns: `id`, `name`, and `email`.

## Supported field types

Toasty supports these Rust types as model fields:

| Rust type | Database type |
|---|---|
| `bool` | Boolean |
| `String` | Text |
| `i8`, `i16`, `i32`, `i64` | Integer (1, 2, 4, 8 bytes) |
| `u8`, `u16`, `u32`, `u64` | Unsigned integer |
| `uuid::Uuid` | UUID |
| `Vec<u8>` | Binary / Blob |
| `Option<T>` | Nullable version of `T` |

With optional feature flags:

| Feature | Rust type | Database type |
|---|---|---|
| `rust_decimal` | `rust_decimal::Decimal` | Decimal |
| `bigdecimal` | `bigdecimal::BigDecimal` | Decimal |
| `jiff` | `jiff::Timestamp` | Timestamp |
| `jiff` | `jiff::civil::Date` | Date |
| `jiff` | `jiff::civil::Time` | Time |
| `jiff` | `jiff::civil::DateTime` | DateTime |

Enable feature flags in your `Cargo.toml`:

```toml
[dependencies]
toasty = { version = "0.1", features = ["sqlite", "jiff"] }
```

## Optional fields

Wrap a field in `Option<T>` to make it nullable:

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,
    bio: Option<String>,
}
```

When creating a record, optional fields default to `None` if not set:

```rust
// bio will be None
let user = User::create()
    .name("Alice")
    .exec(&mut db)
    .await?;
```

## Table names

Toasty auto-pluralizes the struct name to derive the table name. `User` becomes
`users`, `Post` becomes `posts`.

Override the table name with `#[table]`:

```rust
#[derive(Debug, toasty::Model)]
#[table = "people"]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,
}
```

## What gets generated

For a model with basic fields (no relationships or indexes), `#[derive(Model)]`
generates:

**Static methods on the model:**

```rust
// Create a new record
User::create() -> UserCreate

// Create multiple records
User::create_many() -> CreateMany<User>

// Query all records
User::all() -> UserQuery

// Filter records
User::filter(expr) -> UserQuery

// Get field accessors (for building filter expressions)
User::fields() -> UserFields
```

**Instance methods:**

```rust
// Update this record
user.update() -> UserUpdate

// Delete this record
user.delete() -> Delete<User>
```

**Builder structs:**

- `UserCreate` — has a setter method for each field. Call `.exec(&mut db)` to
  insert.
- `UserQuery` — has methods like `.all()`, `.first()`, `.get()`,
  `.collect::<Vec<_>>()` to execute the query.
- `UserUpdate` — has a setter method for each field. Call `.exec(&mut db)` to
  apply changes.

Additional methods are generated when you add attributes like `#[key]`,
`#[unique]`, and `#[index]`. The next chapters cover these.
