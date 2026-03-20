# Indexes and Unique Constraints

Toasty supports two field-level attributes for indexing: `#[unique]` and
`#[index]`. Both create database indexes, but they differ in what gets
generated.

## Unique fields

Add `#[unique]` to a field to create a unique index. On databases that support
unique constraints (SQLite, PostgreSQL, MySQL), the database enforces that no
two records can have the same value for this field. Toasty applies the
constraint on a best-effort basis — if the underlying database does not support
unique indexes (e.g., DynamoDB on non-key attributes), the `#[unique]` attribute
still generates the same query methods but uniqueness is not enforced at the
storage level.

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
```

This generates a unique index on the `email` column:

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_users_email ON users (email);
```

Attempting to insert a duplicate value returns an error:

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
toasty::create!(User {
    name: "Alice",
    email: "alice@example.com",
})
.exec(&mut db)
.await?;

// This fails — email must be unique
let result = toasty::create!(User {
    name: "Bob",
    email: "alice@example.com",
})
.exec(&mut db)
.await;

assert!(result.is_err());
# Ok(())
# }
```

### Generated methods for unique fields

Because a unique field identifies at most one record, Toasty generates a
`get_by_*` method that returns a single record:

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
// Get a single user by email (errors if not found)
let user = User::get_by_email(&mut db, "alice@example.com").await?;
# Ok(())
# }
```

Toasty also generates `filter_by_*`, `update_by_*`, and `delete_by_*` methods:

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
// Filter — returns a query builder
let user = User::filter_by_email("alice@example.com")
    .get(&mut db)
    .await?;

// Update by email
User::update_by_email("alice@example.com")
    .name("Alice Smith")
    .exec(&mut db)
    .await?;

// Delete by email
User::delete_by_email(&mut db, "alice@example.com").await?;
# Ok(())
# }
```

## Indexed fields

Add `#[index]` to a field to tell Toasty that this field is a query target. On
SQL databases, Toasty creates a database index on the column, which lets the
database find matching rows without scanning the entire table. On DynamoDB, the
attribute maps to a secondary index.

Unlike `#[unique]`, `#[index]` does not enforce uniqueness — multiple records
can share the same value.

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[index]
    country: String,
}
```

This generates a non-unique index:

```sql
CREATE INDEX idx_users_country ON users (country);
```

### Generated methods for indexed fields

Because an indexed field may match multiple records, the generated methods work
with collections rather than single records:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[index]
#     country: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// filter_by_country returns a query builder (may match many records)
let users = User::filter_by_country("US")
    .exec(&mut db)
    .await?;

// Update all records matching the index
User::update_by_country("US")
    .country("United States")
    .exec(&mut db)
    .await?;

// Delete all records matching the index
User::delete_by_country(&mut db, "US").await?;
# Ok(())
# }
```

Toasty also generates a `get_by_*` method for indexed fields. It returns the
matching record directly, but errors if no record matches or if more than one
record matches:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[index]
#     country: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let user = User::get_by_country(&mut db, "US").await?;
# Ok(())
# }
```

## Choosing between `#[unique]` and `#[index]`

Both attributes tell Toasty that a field is a query target and generate the same
set of methods: `get_by_*`, `filter_by_*`, `update_by_*`, and `delete_by_*`.

The difference is in the constraint they express:

| Attribute | Meaning | Database effect (SQL) |
|---|---|---|
| `#[unique]` | Each record has a distinct value | `CREATE UNIQUE INDEX` — the database rejects duplicates |
| `#[index]` | Multiple records may share a value | `CREATE INDEX` — no uniqueness enforcement |

Use `#[unique]` for fields that identify a single record — email addresses,
usernames, slugs. Use `#[index]` for fields you query frequently but that
naturally repeat — country, status, category.

## What gets generated

For a model with `#[unique]` on `email` and `#[index]` on `country`:

| Method | Description |
|---|---|
| `User::get_by_email(&mut db, email)` | One record by unique field |
| `User::filter_by_email(email)` | Query builder for unique field |
| `User::update_by_email(email)` | Update builder for unique field |
| `User::delete_by_email(&mut db, email)` | Delete by unique field |
| `User::get_by_country(&mut db, country)` | One record by indexed field |
| `User::filter_by_country(country)` | Query builder for indexed field |
| `User::update_by_country(country)` | Update builder for indexed field |
| `User::delete_by_country(&mut db, country)` | Delete by indexed field |

These methods follow the same patterns as key-generated methods. See
[Querying Records](./querying-records.md),
[Updating Records](./updating-records.md), and
[Deleting Records](./deleting-records.md) for details on terminal methods and
builders.
