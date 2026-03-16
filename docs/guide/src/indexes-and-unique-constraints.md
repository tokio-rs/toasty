# Indexes and Unique Constraints

Toasty supports two field-level attributes for indexing: `#[unique]` and
`#[index]`. Both create database indexes, but they differ in what gets
generated.

## Unique fields

Add `#[unique]` to a field to create a unique index. The database enforces that
no two records can have the same value for this field.

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
User::create()
    .name("Alice")
    .email("alice@example.com")
    .exec(&mut db)
    .await?;

// This fails — email must be unique
let result = User::create()
    .name("Bob")
    .email("alice@example.com")
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

Add `#[index]` to a field to create a non-unique index. This speeds up queries
on the field but does not enforce uniqueness — multiple records can share the
same value.

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
    .all(&mut db)
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

## `#[unique]` vs `#[index]`

Both attributes create a database index and generate `get_by_*`, `filter_by_*`,
`update_by_*`, and `delete_by_*` methods. The difference is in database
enforcement:

| Attribute | Database constraint | Duplicate values |
|---|---|---|
| `#[unique]` | Unique index | Rejected by the database |
| `#[index]` | Non-unique index | Allowed |

Use `#[unique]` when the field must be distinct across all records (emails,
usernames, slugs). Use `#[index]` when you want fast lookups on a field that can
repeat (country, status, category).

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
