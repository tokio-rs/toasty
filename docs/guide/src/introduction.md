# Introduction

Toasty is an async ORM for Rust. It supports both SQL databases (SQLite/Turso,
PostgreSQL, MySQL) and NoSQL databases (DynamoDB).

You define your models as Rust structs and annotate them with
`#[derive(toasty::Model)]`. Toasty infers the database schema from your
annotated structs — field types map to column types, and attributes like
`#[key]`, `#[unique]`, and `#[index]` control the schema. You can customize the
mapping with attributes for table names, column names, and column types. Toasty's
derive macro also generates query builders, create/update/upsert builders, and
relationship accessors at compile time.

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

From this definition, Toasty generates:

- `toasty::create!(User { ... })` — insert new users
- `User::get_by_id()` — fetch a user by primary key
- `User::get_by_email()` — fetch a user by the unique email field
- `User::all()` — query all users
- `user.update()` — a builder for modifying a user
- `user.delete()` — remove a user
- `User::fields()` — field accessors for building filter expressions

The rest of this guide walks through each feature with examples. By the end,
you will know how to define models, set up relationships, query data, and use
Toasty's more advanced features like embedded types, batch operations, and
transactions.

## What this guide covers

- **[Getting Started](./getting-started.md)** — set up a project and run your first query
- **[Defining Models](./defining-models.md)** — struct fields, types, and table mapping
- **[Keys and Auto-Generation](./keys-and-auto-generation.md)** — primary keys, auto-generated values, composite keys
- **[Creating Records](./creating-records.md)** — insert one or many records
- **[Querying Records](./querying-records.md)** — find, filter, and iterate over results
- **[Updating Records](./updating-records.md)** — modify existing records
- **[Upserting Records](./upserting-records.md)** — create or update atomically by a key or unique constraint
- **[Deleting Records](./deleting-records.md)** — remove records
- **[Indexes and Unique Constraints](./indexes-and-unique-constraints.md)** — add indexes and unique constraints
- **[Field Options](./field-options.md)** — column names, types, defaults, and update expressions
- **[Relationships](./relationships.md)** — overview of how models connect to each other
- **[BelongsTo](./belongs-to.md)** — define and use many-to-one relationships
- **[HasMany](./has-many.md)** — define and use one-to-many relationships
- **[Many-to-Many](./many-to-many.md)** — connect two models through a join model
- **[HasOne](./has-one.md)** — define and use one-to-one relationships
- **[Preloading Associations](./preloading-associations.md)** — eager loading to avoid extra queries
- **[Filtering with Expressions](./filtering-with-expressions.md)** — comparisons, AND/OR, and more
- **[Sorting, Limits, and Pagination](./sorting-limits-and-pagination.md)** — order results and paginate
- **[Embedded Types](./embedded-types.md)** — store structs and enums inline
- **[`#[document]` Fields](./document-fields.md)** — store an embedded struct in one document with queryable scalar leaves
- **[JSON Encoding](./json-encoding.md)** — store a serde type or `serde_json::Value` in one opaque column
- **[`Vec<scalar>` Fields](./vec-scalar-fields.md)** — store and query a scalar collection in one field
- **[Batch Operations](./batch-operations.md)** — multiple queries in one round-trip
- **[Transactions](./transactions.md)** — atomic operations
- **[Database Setup](./database-setup.md)** — connection URLs, table creation, and supported databases
- **[Migrations and Schema Management](./schema-management.md)** — create and reset database tables

> **Runnable example:** [`quickstart-blog`] walks the full create → query → update → delete cycle over a `has_many`/`belongs_to` relationship.

[`quickstart-blog`]: https://github.com/tokio-rs/toasty/tree/main/examples/quickstart-blog
