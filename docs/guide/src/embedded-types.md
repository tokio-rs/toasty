# Embedded Types

An embedded type is a struct or enum annotated with `#[derive(toasty::Embed)]`.
Unlike models, embedded types do not get their own database table. Their fields
are stored inline in the parent model's table.

Use embedded types to group related fields without creating a separate table.

## Embedded structs

Define a struct with `#[derive(toasty::Embed)]` and use it as a field in a
model:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Embed)]
struct Address {
    street: String,
    city: String,
}

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,
    address: Address,
}
```

Toasty flattens the embedded struct's fields into the parent table as individual
columns, prefixed with the field name:

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    address_street TEXT NOT NULL,
    address_city TEXT NOT NULL
);
```

The `Address` struct has no table of its own. Its `street` and `city` fields
become `address_street` and `address_city` columns in the `users` table.

### Creating records with embedded structs

Set the embedded field on the create builder by passing an instance of the
struct:

```rust,ignore
let user = toasty::create!(User {
    name: "Alice",
    address: Address {
        street: "123 Main St".to_string(),
        city: "Seattle".to_string(),
    },
})
.exec(&mut db)
.await?;
```

### Updating embedded fields

You can replace the entire embedded struct:

```rust,ignore
user.update()
    .address(Address {
        street: "456 Oak Ave".to_string(),
        city: "Portland".to_string(),
    })
    .exec(&mut db)
    .await?;
```

Or update individual fields within the struct using a closure:

```rust,ignore
user.update()
    .with_address(|a| { a.city("Portland"); })
    .exec(&mut db)
    .await?;
```

The closure receives the embedded struct's update builder, so you only need to
set the fields you want to change.

### Nested embedding

Embedded structs can contain other embedded structs. Each level of nesting adds
another prefix to the column name:

```rust,ignore
#[derive(Debug, toasty::Embed)]
struct Coordinates {
    lat: i64,
    lng: i64,
}

#[derive(Debug, toasty::Embed)]
struct Address {
    street: String,
    city: String,
    coords: Coordinates,
}
```

A `User` model with an `address: Address` field produces columns: `address_street`,
`address_city`, `address_coords_lat`, `address_coords_lng`.

## Embedded enums

Enums annotated with `#[derive(toasty::Embed)]` store a variant discriminant in
the database. Each variant must have an explicit `#[column(variant = N)]`
attribute specifying its integer discriminant value.

### Unit enums

A unit enum (all variants have no fields) maps to a single integer column:

```rust
# use toasty::Model;
#[derive(Debug, PartialEq, toasty::Embed)]
enum Status {
    #[column(variant = 1)]
    Pending,
    #[column(variant = 2)]
    Active,
    #[column(variant = 3)]
    Done,
}

#[derive(Debug, toasty::Model)]
struct Task {
    #[key]
    #[auto]
    id: u64,

    title: String,
    status: Status,
}
```

The `status` column stores the discriminant as an integer:

```sql
CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    status INTEGER NOT NULL   -- 1 = Pending, 2 = Active, 3 = Done
);
```

Use it like any other field:

```rust,ignore
let task = toasty::create!(Task {
    title: "Write docs",
    status: Status::Pending,
})
.exec(&mut db)
.await?;

task.update().status(Status::Done).exec(&mut db).await?;
```

### Data-carrying enums

Enum variants can carry fields. Each variant's fields become nullable columns in
the parent table. Only the active variant's columns are non-null for a given row:

```rust
# use toasty::Model;
#[derive(Debug, PartialEq, toasty::Embed)]
enum ContactInfo {
    #[column(variant = 1)]
    Email { address: String },
    #[column(variant = 2)]
    Phone { number: String },
}

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,
    contact: ContactInfo,
}
```

This produces three columns — one discriminant column plus one nullable column
per variant field:

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    contact INTEGER NOT NULL,         -- discriminant: 1 = Email, 2 = Phone
    contact_address TEXT,             -- non-null when contact = 1
    contact_number TEXT               -- non-null when contact = 2
);
```

Create records by passing enum values:

```rust,ignore
let user = toasty::create!(User {
    name: "Alice",
    contact: ContactInfo::Email {
        address: "alice@example.com".to_string(),
    },
})
.exec(&mut db)
.await?;
```

### Mixed enums

An enum can have both unit variants and data-carrying variants:

```rust,ignore
#[derive(Debug, PartialEq, toasty::Embed)]
enum Status {
    #[column(variant = 1)]
    Pending,
    #[column(variant = 2)]
    Failed { reason: String },
    #[column(variant = 3)]
    Done,
}
```

Unit variants (`Pending`, `Done`) store only the discriminant. The `Failed`
variant also stores its `reason` in a nullable column.

### The `#[column(variant = N)]` attribute

Every variant in an embedded enum must have `#[column(variant = N)]` where `N`
is the integer stored in the database. This is required — Toasty does not
auto-assign discriminant values.

The discriminant values do not need to be sequential. You can choose any `i64`
values, which is useful when adding new variants to an existing schema without
renumbering:

```rust,ignore
#[derive(toasty::Embed)]
enum Priority {
    #[column(variant = 10)]
    Low,
    #[column(variant = 20)]
    Normal,
    #[column(variant = 30)]
    High,
}
```

## Filtering on embedded fields

### Struct fields

Use chained field accessors to filter on embedded struct fields:

```rust,ignore
// Find users in Seattle
let users = User::filter(User::fields().address().city().eq("Seattle"))
    .exec(&mut db)
    .await?;
```

`User::fields().address()` returns the embedded struct's field accessors.
`.city()` returns a field path for the `address_city` column. All comparison
operators (`.eq()`, `.ne()`, `.gt()`, etc.) work on embedded struct fields.

Combine conditions on multiple embedded fields with `.and()`:

```rust,ignore
let users = User::filter(
    User::fields()
        .address()
        .city()
        .eq("Seattle")
        .and(User::fields().address().street().eq("123 Main St")),
)
.exec(&mut db)
.await?;
```

### Enum variants

For embedded enums, Toasty generates `is_*()` methods to filter by variant:

```rust,ignore
// Find all tasks with status = Active
let tasks = Task::filter(Task::fields().status().is_active())
    .exec(&mut db)
    .await?;
```

This compiles to `WHERE status = 2`.

For unit enums, you can also use `.eq()` directly:

```rust,ignore
let tasks = Task::filter(Task::fields().status().eq(Status::Active))
    .exec(&mut db)
    .await?;
```

For data-carrying enums, use `.is_*()` to check the variant and `.matches()` to
filter on the variant's fields:

```rust,ignore
// Find users whose contact is an email with a specific address
let users = User::filter(
    User::fields()
        .contact()
        .email()
        .matches(|e| e.address().eq("alice@example.com")),
)
.exec(&mut db)
.await?;
```

The `.matches()` closure receives the variant's field accessors. It checks both
the discriminant and the field condition.

## Indexing embedded fields

Add `#[index]` or `#[unique]` to fields inside an embedded type. The index
applies to the flattened column in the parent table:

```rust,ignore
#[derive(toasty::Embed)]
struct Contact {
    #[unique]
    email: String,
    #[index]
    country: String,
}

#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,
    contact: Contact,
}
```

This creates a unique index on the `contact_email` column and a non-unique index
on `contact_country`. The same rules from
[Indexes and Unique Constraints](./indexes-and-unique-constraints.md) apply.

Indexes on data-carrying enum variant fields work the same way. The index is
created on the nullable column for that variant's field.
