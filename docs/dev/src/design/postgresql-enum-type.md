# PostgreSQL Native Enum Type

## Overview

Toasty stores embedded enum discriminants as INTEGER or VARCHAR columns. PostgreSQL
users can instead store discriminants using PostgreSQL's native `CREATE TYPE ... AS ENUM`
type. This gives the database awareness of valid values, produces more readable data,
and avoids storing raw integers or unbounded strings for what is a fixed set of choices.

This feature is PostgreSQL-only. Other backends continue to use INTEGER or VARCHAR
discriminants.

## Syntax

Add `variant = "pg_enum"` to the enum-level `#[column]` attribute:

```rust
#[derive(toasty::Embed)]
#[column(variant = "pg_enum")]
enum Status {
    Pending,
    Active,
    Done,
}
```

Each variant maps to a PostgreSQL enum label. By default the label is the Rust
variant's identifier: `'Pending'`, `'Active'`, `'Done'`.

### Explicit labels

Use `#[column(variant = "label")]` on individual variants to control the
PostgreSQL enum label:

```rust
#[derive(toasty::Embed)]
#[column(variant = "pg_enum")]
enum Status {
    #[column(variant = "pending")]
    Pending,
    #[column(variant = "active")]
    Active,
    #[column(variant = "done")]
    Done,
}
```

This stores `'pending'`, `'active'`, `'done'` as the enum labels in PostgreSQL.

### Mixing explicit and default labels

Like string discriminants, explicit labels and defaults can coexist:

```rust
#[derive(toasty::Embed)]
#[column(variant = "pg_enum")]
enum Status {
    #[column(variant = "in_progress")]
    InProgress,      // stored as 'in_progress'
    Done,            // stored as 'Done' (default)
}
```

### Integer discriminants are not allowed

Combining `#[column(variant = "pg_enum")]` with integer variant values is a
compile error. PostgreSQL enum labels are always strings.

```rust
// Compile error: pg_enum variants must use string labels
#[derive(toasty::Embed)]
#[column(variant = "pg_enum")]
enum Status {
    #[column(variant = 1)]  // ERROR
    Pending,
}
```

## Generated SQL Schema

### Type definition

Toasty creates a PostgreSQL enum type named after the Rust enum in snake_case:

```rust
#[derive(toasty::Embed)]
#[column(variant = "pg_enum")]
enum OrderState {
    #[column(variant = "new")]
    New,
    #[column(variant = "shipped")]
    Shipped,
    #[column(variant = "delivered")]
    Delivered,
}
```

```sql
CREATE TYPE order_state AS ENUM ('new', 'shipped', 'delivered');
```

### Column definition

The discriminant column uses the enum type instead of INTEGER or VARCHAR:

```rust
#[derive(toasty::Model)]
struct Order {
    #[key]
    #[auto]
    id: i64,
    state: OrderState,
}
```

```sql
CREATE TABLE orders (
    id BIGSERIAL PRIMARY KEY,
    state order_state NOT NULL
);
```

### Customizing the type name

Use `#[column(type = "name")]` on the enum to override the PostgreSQL type name:

```rust
#[derive(toasty::Embed)]
#[column(variant = "pg_enum", type = "order_status")]
enum OrderState {
    New,
    Shipped,
    Delivered,
}
```

```sql
CREATE TYPE order_status AS ENUM ('New', 'Shipped', 'Delivered');
```

### Data-carrying enums

Data-carrying enums work the same way. The discriminant column uses the enum
type; variant fields remain as separate nullable columns:

```rust
#[derive(toasty::Embed)]
#[column(variant = "pg_enum")]
enum ContactMethod {
    #[column(variant = "email")]
    Email { address: String },
    #[column(variant = "phone")]
    Phone { country: String, number: String },
}
```

```sql
CREATE TYPE contact_method AS ENUM ('email', 'phone');

CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    contact contact_method NOT NULL,
    contact_email_address TEXT,
    contact_phone_country TEXT,
    contact_phone_number TEXT
);
```

## Migrations

### Creating a new enum

When a model using a `pg_enum` is first migrated, Toasty issues `CREATE TYPE`
before `CREATE TABLE`:

```sql
CREATE TYPE status AS ENUM ('pending', 'active', 'done');
CREATE TABLE tasks (
    id BIGSERIAL PRIMARY KEY,
    status status NOT NULL
);
```

### Adding a variant

Adding a new variant to the Rust enum triggers `ALTER TYPE ... ADD VALUE`:

```rust
// Before
enum Status { Pending, Active, Done }

// After
enum Status { Pending, Active, Done, Cancelled }
```

```sql
ALTER TYPE status ADD VALUE 'Cancelled';
```

PostgreSQL appends new values at the end by default. To control ordering, use
the `#[column(after = "label")]` attribute:

```rust
enum Status {
    Pending,
    Active,
    #[column(after = "Active")]
    OnHold,
    Done,
    Cancelled,
}
```

```sql
ALTER TYPE status ADD VALUE 'OnHold' AFTER 'Active';
```

### Renaming a variant

Renaming a variant label requires PostgreSQL 10+. Toasty detects when a variant's
`#[column(variant = "...")]` label changes and the old variant name no longer exists.
Use `#[column(rename_from = "old_label")]` to tell Toasty about the rename:

```rust
#[derive(toasty::Embed)]
#[column(variant = "pg_enum")]
enum Status {
    #[column(variant = "waiting", rename_from = "pending")]
    Pending,
    Active,
    Done,
}
```

```sql
ALTER TYPE status RENAME VALUE 'pending' TO 'waiting';
```

The `rename_from` attribute is only needed during the migration that performs the
rename. It can be removed afterward.

### Removing a variant

PostgreSQL does not support `ALTER TYPE ... DROP VALUE`. Removing a variant from
the Rust enum does not remove the label from the PostgreSQL type. Toasty does not
issue any DDL for removed variants.

If you need to remove a label, you must recreate the type manually outside of
Toasty. This is a PostgreSQL limitation.

### Converting from string or integer discriminants

Switching an existing enum from `#[column(variant = "label")]` (VARCHAR) or
`#[column(variant = N)]` (INTEGER) to `#[column(variant = "pg_enum")]` requires
a migration that:

1. Creates the new enum type
2. Alters the column to use the new type with a USING clause

Toasty generates this migration automatically when the discriminant storage
kind changes:

```sql
CREATE TYPE status AS ENUM ('pending', 'active', 'done');
ALTER TABLE tasks
    ALTER COLUMN status TYPE status USING status::status;
```

For integer-to-pg_enum conversions, the USING clause maps integer values to
labels. Toasty generates a CASE expression:

```sql
CREATE TYPE status AS ENUM ('pending', 'active', 'done');
ALTER TABLE tasks
    ALTER COLUMN status TYPE status USING (
        CASE status
            WHEN 1 THEN 'pending'
            WHEN 2 THEN 'active'
            WHEN 3 THEN 'done'
        END
    )::status;
```

This requires that the old integer-to-label mapping is available. Toasty reads
it from the previous schema snapshot stored in the migration state.

## Querying

The query API is the same as for string and integer discriminants. Toasty
handles the type casting internally:

```rust
// All of these work identically to string/integer discriminants
Task::filter(Task::fields().status().eq(Status::Active))
Task::filter(Task::fields().status().is_pending())
Task::filter(Task::fields().status().ne(Status::Done))
Task::filter(Task::fields().status().in_list([Status::Pending, Status::Active]))
```

### SQL generated for queries

Queries compare against the enum label as a string literal. PostgreSQL
handles the cast from the string literal to the enum type automatically:

```sql
-- .eq(Status::Active)
SELECT * FROM tasks WHERE status = 'Active';

-- .in_list([Status::Pending, Status::Active])
SELECT * FROM tasks WHERE status IN ('Pending', 'Active');
```

No explicit `::status` cast is needed in WHERE clauses because PostgreSQL
infers the type from the column.

### Ordering

PostgreSQL enum values have a sort order defined by their position in the
`CREATE TYPE` statement, not alphabetical order. If you `ORDER BY` an enum
column, rows sort by the declaration order of the labels:

```sql
CREATE TYPE status AS ENUM ('pending', 'active', 'done');

-- ORDER BY status sorts as: pending, active, done
SELECT * FROM tasks ORDER BY status;
```

This matches the order of variants in the Rust enum definition. Reordering
variants in the Rust source does not change the PostgreSQL sort order
(which is fixed at type creation time). Adding a new variant with
`ALTER TYPE ... ADD VALUE` places it at the end unless `AFTER` or `BEFORE`
is specified.

## Inserting

Inserts supply the label as a string literal. PostgreSQL casts it to the
enum type:

```sql
INSERT INTO tasks (status) VALUES ('pending');
```

## Compile-Time Validation

| Condition | Result |
|---|---|
| `#[column(variant = "pg_enum")]` with all string or default labels | Valid |
| `#[column(variant = "pg_enum")]` with integer variant values | Compile error |
| Duplicate labels (including derived defaults) | Compile error |
| Empty string label `#[column(variant = "")]` | Compile error |

## Portability

An enum defined with `#[column(variant = "pg_enum")]` only works with the
PostgreSQL driver. Using it with SQLite, MySQL, or DynamoDB produces a runtime
error when Toasty builds the schema. If you need your enum to work across
multiple backends, use string discriminants instead.

To write backend-portable code that still uses `pg_enum` on PostgreSQL, you
could maintain separate enum definitions per backend. However, the simpler
approach for most applications is to pick one backend and use the discriminant
type that fits it.

## Shared enum types

Multiple models can reference the same `#[column(variant = "pg_enum")]` enum.
Toasty creates the `CREATE TYPE` once and reuses it across tables:

```rust
#[derive(toasty::Embed)]
#[column(variant = "pg_enum")]
enum Priority { Low, Medium, High }

#[derive(toasty::Model)]
struct Task {
    #[key] #[auto] id: i64,
    priority: Priority,
}

#[derive(toasty::Model)]
struct Bug {
    #[key] #[auto] id: i64,
    priority: Priority,
}
```

```sql
CREATE TYPE priority AS ENUM ('Low', 'Medium', 'High');

CREATE TABLE tasks (
    id BIGSERIAL PRIMARY KEY,
    priority priority NOT NULL
);

CREATE TABLE bugs (
    id BIGSERIAL PRIMARY KEY,
    priority priority NOT NULL
);
```

Toasty tracks that the type already exists and does not attempt to create it
twice during migrations.

## Examples

### Unit enum with defaults

```rust
#[derive(Debug, PartialEq, toasty::Embed)]
#[column(variant = "pg_enum")]
enum Color {
    Red,
    Green,
    Blue,
}

#[derive(Debug, toasty::Model)]
struct Widget {
    #[key]
    #[auto]
    id: i64,
    name: String,
    color: Color,
}
```

```sql
CREATE TYPE color AS ENUM ('Red', 'Green', 'Blue');

CREATE TABLE widgets (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    color color NOT NULL
);

-- Insert
INSERT INTO widgets (name, color) VALUES ('Sprocket', 'Red');

-- Query
SELECT * FROM widgets WHERE color = 'Green';
```

### Unit enum with explicit labels

```rust
#[derive(Debug, PartialEq, toasty::Embed)]
#[column(variant = "pg_enum")]
enum Status {
    #[column(variant = "pending")]
    Pending,
    #[column(variant = "active")]
    Active,
    #[column(variant = "done")]
    Done,
}

#[derive(Debug, toasty::Model)]
struct Task {
    #[key]
    #[auto]
    id: i64,
    title: String,
    status: Status,
}
```

```sql
CREATE TYPE status AS ENUM ('pending', 'active', 'done');

CREATE TABLE tasks (
    id BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    status status NOT NULL
);
```

### Data-carrying enum

```rust
#[derive(Debug, PartialEq, toasty::Embed)]
#[column(variant = "pg_enum")]
enum ContactMethod {
    #[column(variant = "email")]
    Email { address: String },
    #[column(variant = "phone")]
    Phone { country: String, number: String },
}

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i64,
    name: String,
    contact: ContactMethod,
}
```

```sql
CREATE TYPE contact_method AS ENUM ('email', 'phone');

CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    contact contact_method NOT NULL,
    contact_email_address TEXT,
    contact_phone_country TEXT,
    contact_phone_number TEXT
);
```

```rust
// Create
let user = User::create()
    .name("Alice")
    .contact(ContactMethod::Email { address: "alice@example.com".into() })
    .exec(&mut db)
    .await?;

// Query
let email_users = User::filter(User::fields().contact().is_email())
    .exec(&mut db)
    .await?;

// Update
user.update()
    .contact(ContactMethod::Phone {
        country: "US".into(),
        number: "555-0100".into(),
    })
    .exec(&mut db)
    .await?;
```
