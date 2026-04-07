# Native Database Enum Types

## Overview

Toasty stores embedded enum discriminants as INTEGER or VARCHAR columns. Databases
that support native enum types — PostgreSQL (`CREATE TYPE ... AS ENUM`) and MySQL
(inline `ENUM(...)` columns) — can store discriminants as typed enum values instead.
This gives the database awareness of valid values, produces more readable data,
and avoids storing raw integers or unbounded strings for what is a fixed set of
choices.

## Syntax

Use `#[column(type = enum)]` on the enum definition to opt into native database
enum storage:

```rust
#[derive(toasty::Embed)]
#[column(type = enum)]
enum Status {
    Pending,
    Active,
    Done,
}
```

Each variant maps to a database enum label. By default the label is the Rust
variant's identifier: `'Pending'`, `'Active'`, `'Done'`.

This follows the same pattern as other explicit column types in Toasty:

```rust
#[column(type = varchar(100))]   // explicit VARCHAR
#[column(type = numeric(28, 10))] // explicit NUMERIC
#[column(type = enum)]            // native database enum
```

### Explicit labels

Use `#[column(variant = "label")]` on individual variants to control the
database enum label:

```rust
#[derive(toasty::Embed)]
#[column(type = enum)]
enum Status {
    #[column(variant = "pending")]
    Pending,
    #[column(variant = "active")]
    Active,
    #[column(variant = "done")]
    Done,
}
```

This stores `'pending'`, `'active'`, `'done'` as the enum labels.

### Mixing explicit and default labels

Like string discriminants, explicit labels and defaults can coexist:

```rust
#[derive(toasty::Embed)]
#[column(type = enum)]
enum Status {
    #[column(variant = "in_progress")]
    InProgress,      // stored as 'in_progress'
    Done,            // stored as 'Done' (default)
}
```

### Integer discriminants are not allowed

Combining `#[column(type = enum)]` with integer variant values is a compile
error. Database enum labels are always strings.

```rust
// Compile error: enum type variants must use string labels
#[derive(toasty::Embed)]
#[column(type = enum)]
enum Status {
    #[column(variant = 1)]  // ERROR
    Pending,
}
```

## Database Support

### PostgreSQL

PostgreSQL uses named enum types. Toasty creates a standalone type with
`CREATE TYPE ... AS ENUM` and references it from column definitions.

### MySQL

MySQL defines enum values inline on the column. There is no standalone named
type. Toasty generates `ENUM('a', 'b', 'c')` as the column type. When the
same Rust enum is used in multiple tables, each table gets its own inline
`ENUM(...)` definition.

### Unsupported backends

SQLite and DynamoDB have no enum type. Using `#[column(type = enum)]` with
these backends produces a runtime error when Toasty builds the schema, the
same as using any other unsupported column type (e.g., `varchar` on SQLite).

## Generated SQL Schema

### PostgreSQL

Toasty creates a PostgreSQL enum type named after the Rust enum in snake_case:

```rust
#[derive(toasty::Embed)]
#[column(type = enum)]
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

#### Customizing the PostgreSQL type name

The `#[column(type = ...)]` attribute already controls the database type. To
specify a custom name for the PostgreSQL enum type, use `enum` with a name
argument:

```rust
#[derive(toasty::Embed)]
#[column(type = enum("order_status"))]
enum OrderState {
    New,
    Shipped,
    Delivered,
}
```

```sql
CREATE TYPE order_status AS ENUM ('New', 'Shipped', 'Delivered');
```

Without a name argument, Toasty derives the type name from the Rust enum
name in snake_case.

### MySQL

MySQL enum types are defined inline on the column:

```rust
#[derive(toasty::Embed)]
#[column(type = enum)]
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
CREATE TABLE orders (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    state ENUM('new', 'shipped', 'delivered') NOT NULL
);
```

The `enum("name")` syntax is ignored on MySQL since there is no standalone
type to name.

### Data-carrying enums

Data-carrying enums work the same way on both backends. The discriminant
column uses the enum type; variant fields remain as separate nullable columns:

```rust
#[derive(toasty::Embed)]
#[column(type = enum)]
enum ContactMethod {
    #[column(variant = "email")]
    Email { address: String },
    #[column(variant = "phone")]
    Phone { country: String, number: String },
}
```

PostgreSQL:
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

MySQL:
```sql
CREATE TABLE users (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    contact ENUM('email', 'phone') NOT NULL,
    contact_email_address TEXT,
    contact_phone_country TEXT,
    contact_phone_number TEXT
);
```

## Migrations

### Creating a new enum

When a model using `#[column(type = enum)]` is first migrated, Toasty issues
the appropriate DDL before `CREATE TABLE`.

PostgreSQL:
```sql
CREATE TYPE status AS ENUM ('pending', 'active', 'done');
CREATE TABLE tasks (
    id BIGSERIAL PRIMARY KEY,
    status status NOT NULL
);
```

MySQL:
```sql
CREATE TABLE tasks (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    status ENUM('pending', 'active', 'done') NOT NULL
);
```

### Label ordering

Database enum types have a declaration order that affects `ORDER BY` behavior.
Toasty manages this order with two rules:

1. **Initial creation**: Labels are ordered by the Rust enum's variant
   declaration order.
2. **Subsequent migrations**: Toasty preserves the existing label order from
   the previous schema snapshot. New variants are appended at the end.
   Reordering variants in the Rust source does not trigger any DDL and does
   not change the database label order.

This means the label order is a one-time decision made at creation. If you
need to change the order later, you must do so manually outside of Toasty
(by recreating the type on PostgreSQL, or running a `MODIFY COLUMN` on
MySQL).

### Adding a variant

Adding a new variant to the Rust enum:

```rust
// Before
enum Status { Pending, Active, Done }

// After
enum Status { Pending, Active, Done, Cancelled }
```

New variants are appended after all existing labels, regardless of where
they appear in the Rust enum definition.

PostgreSQL:
```sql
ALTER TYPE status ADD VALUE 'Cancelled';
```

MySQL:
```sql
ALTER TABLE tasks MODIFY COLUMN status
    ENUM('pending', 'active', 'done', 'cancelled') NOT NULL;
```

MySQL requires rewriting the full enum definition on every change. Toasty
handles this automatically, preserving the existing label order and
appending the new label at the end.

### Renaming a variant

Use `#[column(rename_from = "old_label")]` to tell Toasty about the rename:

```rust
#[derive(toasty::Embed)]
#[column(type = enum)]
enum Status {
    #[column(variant = "waiting", rename_from = "pending")]
    Pending,
    Active,
    Done,
}
```

PostgreSQL (requires PostgreSQL 10+):
```sql
ALTER TYPE status RENAME VALUE 'pending' TO 'waiting';
```

MySQL:
```sql
ALTER TABLE tasks MODIFY COLUMN status
    ENUM('waiting', 'active', 'done') NOT NULL;
```

On MySQL, the `MODIFY COLUMN` preserves the existing label order, substituting
the renamed label in its original position.

The `rename_from` attribute is only needed during the migration that performs
the rename. It can be removed afterward.

### Removing a variant

Toasty does not support removing enum variants. Removing a variant from the
Rust enum while the label still exists in the database schema is a migration
error. Destructive schema changes like this require a broader design for
handling data loss scenarios and are out of scope for this feature.

### Converting from string or integer discriminants

Switching an existing enum from `#[column(variant = "label")]` (VARCHAR) or
`#[column(variant = N)]` (INTEGER) to `#[column(type = enum)]` requires a
migration that creates the enum type and converts the column.

PostgreSQL — converting from VARCHAR:
```sql
CREATE TYPE status AS ENUM ('pending', 'active', 'done');
ALTER TABLE tasks
    ALTER COLUMN status TYPE status USING status::status;
```

PostgreSQL — converting from INTEGER:
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

The integer-to-label mapping comes from the previous schema snapshot stored
in the migration state.

MySQL — converting from VARCHAR:
```sql
ALTER TABLE tasks MODIFY COLUMN status
    ENUM('pending', 'active', 'done') NOT NULL;
```

MySQL — converting from INTEGER:
```sql
ALTER TABLE tasks MODIFY COLUMN status
    ENUM('pending', 'active', 'done') NOT NULL;
```

MySQL's `MODIFY COLUMN` handles the type change. For integer conversions,
Toasty issues an intermediate step to map integers to their label strings
before converting the column type.

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

Queries compare against the enum label as a string literal. Both PostgreSQL
and MySQL handle the cast from the string literal to the enum type
automatically:

```sql
-- .eq(Status::Active)
SELECT * FROM tasks WHERE status = 'Active';

-- .in_list([Status::Pending, Status::Active])
SELECT * FROM tasks WHERE status IN ('Pending', 'Active');
```

No explicit cast is needed in WHERE clauses because the database infers the
type from the column.

### Ordering

Toasty does not support ordering comparisons (`>`, `<`, etc.) on enum fields.
The query API provides `eq`, `ne`, `in_list`, and variant checks only.

Both PostgreSQL and MySQL define a sort order for enum values based on their
position in the type definition, not alphabetically. Toasty does not expose
or manage this ordering. Users who query the database directly should be
aware that `ORDER BY` on an enum column uses declaration order, not
lexicographic order.

## Inserting

Inserts supply the label as a string literal. The database casts it to the
enum type:

```sql
INSERT INTO tasks (status) VALUES ('pending');
```

## Compile-Time Validation

| Condition | Result |
|---|---|
| `#[column(type = enum)]` with all string or default labels | Valid |
| `#[column(type = enum)]` with integer variant values | Compile error |
| Duplicate labels (including derived defaults) | Compile error |
| Empty string label `#[column(variant = "")]` | Compile error |
| Label longer than 63 bytes (PostgreSQL limit) | Compile error |

## Portability

`#[column(type = enum)]` works with PostgreSQL and MySQL. Using it with SQLite
or DynamoDB produces a runtime error when Toasty builds the schema.

If you need your enum to work across all backends, use string discriminants
instead (`#[column(variant = "label")]` without `#[column(type = enum)]`).

## Shared enum types

Multiple models can reference the same `#[column(type = enum)]` enum.

On PostgreSQL, Toasty creates the `CREATE TYPE` once and reuses it across
tables:

```rust
#[derive(toasty::Embed)]
#[column(type = enum)]
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

PostgreSQL:
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

MySQL:
```sql
CREATE TABLE tasks (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    priority ENUM('Low', 'Medium', 'High') NOT NULL
);

CREATE TABLE bugs (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    priority ENUM('Low', 'Medium', 'High') NOT NULL
);
```

Toasty tracks that the PostgreSQL type already exists and does not attempt to
create it twice during migrations. On MySQL each table carries its own inline
definition.

## Examples

### Unit enum with defaults

```rust
#[derive(Debug, PartialEq, toasty::Embed)]
#[column(type = enum)]
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

PostgreSQL:
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
#[column(type = enum)]
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

PostgreSQL:
```sql
CREATE TYPE status AS ENUM ('pending', 'active', 'done');

CREATE TABLE tasks (
    id BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    status status NOT NULL
);
```

MySQL:
```sql
CREATE TABLE tasks (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    title TEXT NOT NULL,
    status ENUM('pending', 'active', 'done') NOT NULL
);
```

### Data-carrying enum

```rust
#[derive(Debug, PartialEq, toasty::Embed)]
#[column(type = enum)]
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
