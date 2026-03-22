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
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     bio: Option<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// bio will be NULL in the database, None in Rust
let user = toasty::create!(User { name: "Alice" })
    .exec(&mut db)
    .await?;
# Ok(())
# }
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
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
# }
# fn __example() {
// Returns a create builder (usually called via the toasty::create! macro)
# let _ =
User::create();

// Returns a builder for bulk inserts
# let _ =
User::create_many();

// Returns a query builder for all records
# let _ =
User::all();

// Returns a query builder with a filter applied
# let _ =
User::filter(User::fields().name().eq("Alice"));

// Returns field accessors (for building filter expressions)
# let _ =
User::fields();
# }
```

**Instance methods:**

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
# }
# fn __example(mut user: User) {
// Returns an update builder for this record
# let _ =
user.update();

// Returns a delete builder for this record
# let _ =
user.delete();
# }
```

**Builders:**

- The **create builder** is typically used through the `toasty::create!` macro,
  which provides struct-literal syntax:

  ```rust
  # use toasty::Model;
  # #[derive(Debug, toasty::Model)]
  # struct User {
  #     #[key]
  #     #[auto]
  #     id: u64,
  #     name: String,
  #     email: String,
  # }
  # async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
  let user = toasty::create!(User {
      name: "Alice",
      email: "alice@example.com",
  })
  .exec(&mut db)
  .await?;
  # Ok(())
  # }
  ```

- The **update builder** returned by `user.update()` has a setter for each
  field. Only set the fields you want to change:

  ```rust
  # use toasty::Model;
  # #[derive(Debug, toasty::Model)]
  # struct User {
  #     #[key]
  #     #[auto]
  #     id: u64,
  #     name: String,
  #     email: String,
  # }
  # async fn __example(mut user: User, mut db: toasty::Db) -> toasty::Result<()> {
  user.update()
      .name("Bob")
      .exec(&mut db)
      .await?;
  # Ok(())
  # }
  ```

- The **query builder** returned by `User::all()` or `User::filter()` has
  methods like `.exec()`, `.first()`, `.get()`, and `.collect::<Vec<_>>()` to
  execute the query.

### What types can you pass to setters?

Builder setters accept more than just the exact field type. For a `String`
field, you can pass a `String`, a `&str`, or even an `Option<String>`. For
numeric fields, you can pass the value directly or a reference. This works
through Toasty's `IntoExpr` trait, which handles the conversion automatically.

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
# }
# fn __example() {
// String literal (&str)
# let _ =
toasty::create!(User { name: "Alice" });

// Owned String
let name = "Bob".to_string();
# let _ =
toasty::create!(User { name: name });

// Reference to a String
let name = "Carol".to_string();
# let _ =
toasty::create!(User { name: &name });
# }
```

You don't need to call `.to_string()` or `.clone()` to satisfy the setter —
pass the value in whatever form you have it.

Additional methods are generated when you add attributes like `#[key]`,
`#[unique]`, and `#[index]`. The next chapters cover these.
