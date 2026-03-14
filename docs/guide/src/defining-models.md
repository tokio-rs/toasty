# Defining Models

A model is a Rust struct annotated with `#[derive(toasty::Model)]`. Each struct
maps to a database table and each field maps to a column.

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,
    email: String,
}
```

This defines a `User` model that maps to a `users` table with three columns.
In SQLite, the generated table looks like:

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT NOT NULL
);
```

Each struct field becomes a column. Required fields (`String`, `u64`, etc.) map
to `NOT NULL` columns. The `#[key]` attribute marks the primary key, and
`#[auto]` tells Toasty to auto-generate the value on insert.

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

Wrap a field in `Option<T>` to make it nullable. An `Option<T>` field maps to a
nullable column in the database — the column allows `NULL` values instead of
requiring `NOT NULL`.

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,
    bio: Option<String>,
}
```

The `bio` field produces a nullable column:

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    bio TEXT          -- nullable, allows NULL
);
```

When creating a record, optional fields default to `NULL` if not set:

```rust
// bio will be NULL in the database, None in Rust
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
    id: u64,

    name: String,
}
```

This maps to a table named `people` instead of the default `users`.

## What gets generated

For a model with basic fields (no relationships or indexes), `#[derive(Model)]`
generates:

**Static methods on the model:**

```rust
// Returns a create builder
User::create()

// Returns a builder for bulk inserts
User::create_many()

// Returns a query builder for all records
User::all()

// Returns a query builder with a filter applied
User::filter(expr)

// Returns field accessors (for building filter expressions)
User::fields()
```

**Instance methods:**

```rust
// Returns an update builder for this record
user.update()

// Returns a delete builder for this record
user.delete()
```

**Builders:**

- The **create builder** returned by `User::create()` has a setter method for
  each field. Call `.exec(&mut db)` to insert.
- The **query builder** returned by `User::all()` or `User::filter()` has
  methods like `.all()`, `.first()`, `.get()`, and `.collect::<Vec<_>>()` to
  execute the query.
- The **update builder** returned by `user.update()` has a setter method for
  each field. Call `.exec(&mut db)` to apply changes.

Additional methods are generated when you add attributes like `#[key]`,
`#[unique]`, and `#[index]`. The next chapters cover these.
