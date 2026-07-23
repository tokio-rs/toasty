# Embedded Types

An embedded type is a struct or enum annotated with `#[derive(toasty::Embed)]`.
Unlike models, embedded types do not get their own database table. Their fields
are stored inline in the parent model's table.

Use embedded types to group related fields without creating a separate table.

## Newtype structs

A newtype struct is a single-field tuple struct like `struct Email(String)`.
Annotate it with `#[derive(toasty::Embed)]` to use it as a model field:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Embed)]
struct Email(String);

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,
    email: Email,
}
```

Unlike multi-field embedded structs, a newtype maps to a single column with the
parent field's name — no prefix is added:

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT NOT NULL     -- not "email_0"
);
```

Use newtypes to add type safety to primitive fields. An `Email` and a `Username`
are both strings, but the type system prevents mixing them up:

```rust,ignore
let user = toasty::create!(User {
    name: "Alice",
    email: Email("alice@example.com".into()),
})
.exec(&mut db)
.await?;

assert_eq!(user.email.0, "alice@example.com");
```

Newtypes support the same operations as primitive fields — filtering, updating,
`#[key]`, `#[unique]`, and `#[index]` all work:

```rust,ignore
// Filter by newtype field
let users = User::filter(User::fields().email().eq(Email("alice@example.com".into())))
    .exec(&mut db)
    .await?;

// Update a newtype field
user.update()
    .email(Email("new@example.com".into()))
    .exec(&mut db)
    .await?;
```

A newtype can also be used as a primary key:

```rust,ignore
#[derive(Debug, toasty::Embed)]
struct UserId(String);

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: UserId,
    name: String,
}
```

### Newtype with `#[unique]` and `#[index]`

Place `#[unique]` or `#[index]` on the model field (not inside the newtype):

```rust,ignore
#[derive(Debug, toasty::Embed)]
struct Email(String);

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[unique]
    email: Email,
}
```

This generates the same methods as a primitive unique field —
`User::get_by_email()`, `User::filter_by_email()`, etc.

### Newtypes inside embedded structs

Newtypes can be nested inside multi-field embedded structs:

```rust,ignore
#[derive(Debug, toasty::Embed)]
struct ZipCode(String);

#[derive(Debug, toasty::Embed)]
struct Address {
    city: String,
    zip: ZipCode,
}
```

The `ZipCode` field inside `Address` produces a single column (`address_zip`,
not `address_zip_0`). Filtering works through the normal chained accessors:

```rust,ignore
let users = User::filter(User::fields().address().zip().eq(ZipCode("98101".into())))
    .exec(&mut db)
    .await?;
```

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

Or patch individual fields within the struct with `stmt::patch`:

```rust,ignore
use toasty::stmt;

user.update()
    .address(stmt::patch(Address::fields().city(), "Portland"))
    .exec(&mut db)
    .await?;
```

`stmt::patch` targets a sub-field by its typed path and leaves the other
fields of the embedded struct unchanged. Combine multiple sub-field updates
with `stmt::apply`:

```rust,ignore
user.update()
    .address(stmt::apply([
        stmt::patch(Address::fields().street(), "456 Oak Ave"),
        stmt::patch(Address::fields().city(), "Portland"),
    ]))
    .exec(&mut db)
    .await?;
```

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
the database. By default, Toasty derives a string label for each variant by
converting its Rust name to `snake_case`.

### Unit enums

A unit enum (all variants have no fields) maps to a single column:

```rust
# use toasty::Model;
#[derive(Debug, PartialEq, toasty::Embed)]
enum Status {
    Pending,
    Active,
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

The `status` column stores `pending`, `active`, or `done`. PostgreSQL uses a
named enum type, MySQL uses `ENUM`, and SQLite uses `TEXT` with a check
constraint. DynamoDB stores the label as a string attribute.

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
    Email { address: String },
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

This produces three columns: one discriminant column containing `email` or
`phone`, plus nullable `contact_address` and `contact_number` columns. Only the
field for the active variant contains a value.

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
enum ImportStatus {
    Pending,
    Failed { reason: String },
    Done,
}
```

Unit variants (`Pending`, `Done`) store only the discriminant. The `Failed`
variant also stores its `reason` in a nullable column.

### Changing stored discriminants

Without additional attributes, Toasty converts each variant name to
`snake_case`. For example, `PreferredSupplier` uses the label
`preferred_supplier`.

Add `#[column(rename_all = "...")]` to the enum to select another naming rule:

```rust
#[derive(toasty::Embed)]
#[column(rename_all = "SCREAMING_SNAKE_CASE")]
enum PartyKind {
    Customer,
    PreferredSupplier,
}
```

This enum uses `CUSTOMER` and `PREFERRED_SUPPLIER`. Toasty accepts the following
rules:

| Rule | `PreferredSupplier` label |
| --- | --- |
| `lowercase` | `preferredsupplier` |
| `UPPERCASE` | `PREFERREDSUPPLIER` |
| `PascalCase` | `PreferredSupplier` |
| `camelCase` | `preferredSupplier` |
| `snake_case` | `preferred_supplier` |
| `SCREAMING_SNAKE_CASE` | `PREFERRED_SUPPLIER` |
| `kebab-case` | `preferred-supplier` |
| `SCREAMING-KEBAB-CASE` | `PREFERRED-SUPPLIER` |

Set individual labels with `#[column(variant = "...")]`:

```rust
#[derive(toasty::Embed)]
enum PartyKind {
    #[column(variant = "customer")]
    Customer,
    #[column(variant = "preferred-supplier")]
    PreferredSupplier,
}
```

An explicit variant label takes precedence over `rename_all` when an enum uses
both attributes. `rename_all` changes variant labels only; it does not change
the database enum type name. It applies whether the labels use native enum
storage or a plain `text` or `varchar` column.

#### Integer discriminants

To store integer discriminants, set `#[column(variant = N)]` on every variant.
Toasty does not auto-assign integers. The values do not need to be sequential;
you can choose any non-negative `i64` values. This lets you add variants to an
existing schema without renumbering:

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

Integer discriminants use `i64` storage by default. Add an integer
`#[column(type = ...)]` to the enum to request a narrower database type for
each flattened discriminant column and each `Vec<unit-enum>` element:

```rust,ignore
#[derive(toasty::Embed)]
#[column(type = u8)]
enum Priority {
    #[column(variant = 10)]
    Low,
    #[column(variant = 20)]
    Normal,
    #[column(variant = 30)]
    High,
}
```

This requests unsigned 8-bit storage. MySQL uses `TINYINT UNSIGNED`; databases
without an unsigned 8-bit integer use their closest supported representation.
The type follows the enum through flattened embedded structs and transparent
field wrappers such as `Option`, `Deferred`, `Box`, `Arc`, and `Rc`.

Place the attribute on a model field to override the enum-level default for
that use. On `Vec<unit-enum>`, the field-level type describes each element:

```rust,ignore
#[derive(toasty::Model)]
struct Task {
    #[key]
    id: u64,

    #[column(type = u8)]
    priority: Priority,

    #[column(type = u16)]
    recent_priorities: Vec<Priority>,
}
```

Every discriminant must fit both the enum-level type and any narrower
field-level override. [`#[document]` storage](./document-fields.md) currently
rejects enum embeds because enum document encoding is not supported.

An enum cannot mix string and integer discriminants. Integer-discriminant enums
do not support `rename_all`.

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

This filters on the stored `active` label (`WHERE status = 'active'` in SQL).

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

> **Runnable example:** [`crm-embedded`] flattens embedded structs and enums, keys a model with a newtype, and patches embedded fields.

[`crm-embedded`]: https://github.com/tokio-rs/toasty/tree/main/examples/crm-embedded
