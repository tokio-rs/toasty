# Getting Started

This chapter walks through creating a project, defining a model, and running
your first queries.

## Create a new project

```bash
cargo new my-app
cd my-app
```

Add the following dependencies to `Cargo.toml`:

```toml
[dependencies]
toasty = { version = "0.1", features = ["sqlite"] }
tokio = { version = "1", features = ["full"] }
```

The `sqlite` feature enables the SQLite driver. Toasty also supports
`postgresql`, `mysql`, and `dynamodb` — swap the feature flag to use a
different database.

## Define a model

Replace the contents of `src/main.rs` with:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[unique]
    email: String,
}

# async fn __example() -> toasty::Result<()> {
#[tokio::main]
async fn main() -> toasty::Result<()> {
    // Build a Db handle, registering all models
    let mut db = toasty::Db::builder()
        .register::<User>()
        .connect("sqlite::memory:")
        .await?;

    // Create tables based on registered models
    db.push_schema().await?;

    // Create a user
    let user = toasty::create!(User {
        name: "Alice",
        email: "alice@example.com",
    })
    .exec(&mut db)
    .await?;

    println!("Created: {:?}", user.name);

    // Fetch the user back by primary key
    let found = User::get_by_id(&mut db, &user.id).await?;
    println!("Found: {:?}", found.email);

    Ok(())
}
# Ok(())
# }
```

Run it:

```bash
cargo run
```

You should see:

```text
Created: "Alice"
Found: "alice@example.com"
```

## What just happened

The `#[derive(toasty::Model)]` macro read the `User` struct and generated
several types and methods at compile time:

| You wrote | Toasty generated |
|---|---|
| `struct User` | `toasty::create!(User { ... })` — insert rows |
| `#[key]` on `id` | `User::get_by_id()` — fetch by primary key |
| `#[auto]` on `id` | Auto-generates an ID when you create a user |
| `#[unique]` on `email` | `User::get_by_email()` — fetch by email |

You did not write any of these methods. They come from the derive macro. The
rest of this guide shows everything the macro can generate and how to use it.

## Connecting to a database

`Db::builder()` creates a builder where you register your models and then
connect to a database. Every model must be registered before connecting so that
Toasty can infer the full database schema — tables, columns, indexes, and
relationships between models.

```rust,ignore
let mut db = toasty::Db::builder()
    .register::<User>()
    .register::<Post>()
    .connect("sqlite::memory:")
    .await?;
```

The connection URL determines which database driver to use. See
[Database Setup](./database-setup.md) for connection URLs for each
supported database.

## Creating tables

`db.push_schema()` creates all tables and indexes defined by your registered
models. See [Schema Management](./schema-management.md) for more on managing
your database schema.
