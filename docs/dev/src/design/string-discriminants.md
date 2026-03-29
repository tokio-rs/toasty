# String Discriminants for Embedded Enums

## Overview

Embedded enums currently store their discriminant as an integer column using
`#[column(variant = N)]`. String discriminants let users write
`#[column(variant = "label")]` instead, storing the discriminant as a TEXT column.
This works across all supported databases (SQLite, PostgreSQL, MySQL, DynamoDB).

This document covers the public-facing API only. PostgreSQL native enum types are
a separate feature and not covered here.

## Syntax

### Explicit string labels

Use a string literal in the `variant` attribute:

```rust
#[derive(toasty::Embed)]
enum Status {
    #[column(variant = "pending")]
    Pending,
    #[column(variant = "active")]
    Active,
    #[column(variant = "done")]
    Done,
}
```

The discriminant column stores `"pending"`, `"active"`, or `"done"` as TEXT.

### Default labels (omit variant attribute)

When all variants omit `#[column(variant = ...)]`, Toasty derives string labels
automatically by converting each variant name to `snake_case`:

```rust
#[derive(toasty::Embed)]
enum Status {
    Pending,        // → "pending"
    Active,         // → "active"
    AlmostDone,     // → "almost_done"
}
```

This is equivalent to writing `#[column(variant = "pending")]` on each variant.

### Mixing explicit and default labels

Variants with explicit string labels and variants without any label can coexist.
Unlabeled variants use the default `snake_case` derivation:

```rust
#[derive(toasty::Embed)]
enum Status {
    #[column(variant = "waiting")]
    Pending,          // stored as "waiting"
    Active,           // stored as "active" (default)
    Done,             // stored as "done" (default)
}
```

All variants in a single enum must use the same discriminant kind. Integer and
string discriminants cannot be mixed. If any variant uses a string label or if
any variant omits the `variant` attribute entirely, the enum uses string
discriminants.

## Generated SQL Schema

Integer discriminants produce an INTEGER column. String discriminants produce a
TEXT column.

### Unit enum with integer discriminants (existing behavior)

```rust
#[derive(toasty::Embed)]
enum Status {
    #[column(variant = 1)]
    Pending,
    #[column(variant = 2)]
    Active,
}
```

```sql
CREATE TABLE task (
    id INTEGER PRIMARY KEY,
    status INTEGER NOT NULL
);
```

### Unit enum with string discriminants

```rust
#[derive(toasty::Embed)]
enum Status {
    #[column(variant = "pending")]
    Pending,
    #[column(variant = "active")]
    Active,
}
```

```sql
CREATE TABLE task (
    id INTEGER PRIMARY KEY,
    status TEXT NOT NULL
);
```

### Data-carrying enum with string discriminants

```rust
#[derive(toasty::Embed)]
enum ContactMethod {
    #[column(variant = "email")]
    Email { address: String },
    #[column(variant = "phone")]
    Phone { country: String, number: String },
}
```

```sql
CREATE TABLE user (
    id INTEGER PRIMARY KEY,
    contact TEXT NOT NULL,
    contact_email_address TEXT,
    contact_phone_country TEXT,
    contact_phone_number TEXT
);
```

The discriminant column type changes from INTEGER to TEXT. All other columns
(variant data fields) remain the same.

## Querying

The query API is unchanged. Variant checks, field access, and `.matches()` work
the same way regardless of whether the discriminant is an integer or string.

```rust
// Filter by variant
Task::all().filter(Task::FIELDS.status().is_pending())

// Equivalent using .matches()
Task::all().filter(
    Task::FIELDS.status().matches(Status::VARIANTS.pending())
)

// Match variant and access fields
User::all().filter(
    User::FIELDS.contact().matches(
        ContactMethod::VARIANTS.email().address().contains("@gmail")
    )
)
```

Toasty generates the correct comparison value (`WHERE status = 'pending'` vs
`WHERE status = 1`) based on the discriminant kind.

## Creating

Creating records uses the same API. Toasty writes the correct discriminant value
to the database:

```rust
Task::create()
    .status(Status::Pending)
    .exec(&db).await?;
// Inserts status = 'pending' (string) instead of status = 1 (integer)
```

## Updating

Updating works the same way for both discriminant kinds:

```rust
// Whole-variant replacement
user.update()
    .contact(ContactMethod::Email { address: "new@example.com".into() })
    .exec(&db).await?;

// Partial update within a variant
user.update()
    .with_contact(|c| {
        c.phone(|p| {
            p.with_number(|n| n.set("555-1234"));
        });
    })
    .exec(&db).await?;
```

## Compile-Time Validation

The `#[derive(toasty::Embed)]` macro validates discriminant attributes at
compile time.

| Condition | Result |
|---|---|
| All variants have `#[column(variant = N)]` (integer) | Integer discriminants |
| All variants have `#[column(variant = "label")]` (string) | String discriminants |
| All variants omit `#[column(variant = ...)]` | String discriminants (default `snake_case` labels) |
| Some variants have string labels, others omit the attribute | String discriminants (omitted variants use defaults) |
| Mix of integer and string `variant` values | Compile error |
| Duplicate integer discriminant values | Compile error |
| Duplicate string labels (including derived defaults) | Compile error |
| Empty string label `#[column(variant = "")]` | Compile error |

## Examples

### Basic unit enum with defaults

```rust
#[derive(Model)]
struct Task {
    #[key]
    #[auto]
    id: u64,
    status: Status,
}

#[derive(toasty::Embed)]
enum Status {
    Pending,
    Active,
    Done,
}
```

```sql
CREATE TABLE task (
    id INTEGER PRIMARY KEY,
    status TEXT NOT NULL
);
-- Values: 'pending', 'active', 'done'
```

### Unit enum with explicit string labels

```rust
#[derive(Model)]
struct Order {
    #[key]
    #[auto]
    id: u64,
    state: OrderState,
}

#[derive(toasty::Embed)]
enum OrderState {
    #[column(variant = "new")]
    New,
    #[column(variant = "in_progress")]
    InProgress,
    #[column(variant = "shipped")]
    Shipped,
    #[column(variant = "delivered")]
    Delivered,
}
```

```sql
CREATE TABLE order (
    id INTEGER PRIMARY KEY,
    state TEXT NOT NULL
);
-- Values: 'new', 'in_progress', 'shipped', 'delivered'
```

### Data-carrying enum with string discriminants

```rust
#[derive(Model)]
struct User {
    #[key]
    #[auto]
    id: u64,
    contact: ContactMethod,
}

#[derive(toasty::Embed)]
enum ContactMethod {
    #[column(variant = "email")]
    Email { address: String },
    #[column(variant = "phone")]
    Phone { country: String, number: String },
    #[column(variant = "mail")]
    Mail { address: Address },
}

#[derive(toasty::Embed)]
struct Address {
    street: String,
    city: String,
}
```

```sql
CREATE TABLE user (
    id INTEGER PRIMARY KEY,
    contact TEXT NOT NULL,
    contact_email_address TEXT,
    contact_phone_country TEXT,
    contact_phone_number TEXT,
    contact_mail_address_street TEXT,
    contact_mail_address_city TEXT
);
-- Discriminant values: 'email', 'phone', 'mail'
```

Querying, creating, and updating use the same API as integer discriminants:

```rust
// Create
User::create()
    .contact(ContactMethod::Email { address: "alice@example.com".into() })
    .exec(&db).await?;

// Query
User::all().filter(User::FIELDS.contact().is_email())
User::all().filter(
    User::FIELDS.contact().matches(
        ContactMethod::VARIANTS.phone().country().eq("US")
    )
)

// Update
user.update()
    .contact(ContactMethod::Phone {
        country: "US".into(),
        number: "555-0100".into(),
    })
    .exec(&db).await?;
```
