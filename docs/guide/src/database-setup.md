# Database Setup

Opening a database connection in Toasty has two steps: register your models,
then connect to a database. `Db::builder()` handles both.

```rust,ignore
let mut db = toasty::Db::builder()
    .models(toasty::models!(User, Post))
    .connect("sqlite::memory:")
    .await?;
```

## Registering models

The `models!` macro builds a `ModelSet` — the collection of model definitions
Toasty uses to generate the database schema. It accepts three forms, which can
be combined freely:

```rust,ignore
toasty::models!(
    // All models from the current crate
    crate::*,
    // All models from an external crate
    third_party_models::*,
    // Individual models
    User,
    other_module::Post,
)
```

`crate::*` finds all `#[derive(Model)]` and `#[derive(Embed)]` types in your
crate at compile time. This is the simplest option when all your models live in
one crate.

You don't need to list every model. Registering a model also registers any
models reachable through its fields — `BelongsTo`, `HasMany`, `HasOne`, and
embedded types are all discovered by traversing the model's fields. For
example, if `User` has a `HasMany<Post>` field and `Post` has a `BelongsTo<User>`
field, `toasty::models!(User)` registers both `User` and `Post`.

## Connection URLs

The connection URL determines which database driver Toasty uses. Each driver
requires its corresponding feature flag in `Cargo.toml`.

| Scheme | Database | Feature flag |
|---|---|---|
| `sqlite` | SQLite | `sqlite` |
| `postgresql` or `postgres` | PostgreSQL | `postgresql` |
| `mysql` | MySQL | `mysql` |
| `dynamodb` | DynamoDB | `dynamodb` |

Examples:

```rust,ignore
// In-memory SQLite
.connect("sqlite::memory:")

// SQLite file
.connect("sqlite://path/to/db.sqlite")

// PostgreSQL
.connect("postgresql://user:pass@localhost:5432/mydb")

// MySQL
.connect("mysql://user:pass@localhost:3306/mydb")

// DynamoDB (uses AWS config from environment)
.connect("dynamodb://us-east-1")
```

## Using a driver directly

If you need more control over the driver configuration, construct the driver
yourself and pass it to `build()` instead of `connect()`:

```rust,ignore
let driver = toasty_driver_sqlite::Sqlite::in_memory();
let mut db = toasty::Db::builder()
    .models(toasty::models!(User))
    .build(driver)
    .await?;
```

## Table name prefix

To prefix all generated table names (useful when multiple services share a
database), call `table_name_prefix()` on the builder:

```rust,ignore
let mut db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .table_name_prefix("myapp_")
    .connect("sqlite::memory:")
    .await?;
```
