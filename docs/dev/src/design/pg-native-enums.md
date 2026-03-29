# PostgreSQL Native Enum Mapping

Rust enums derived with `#[derive(toasty::Embed)]` currently store their
discriminant as an integer column in all databases. PostgreSQL supports native
`CREATE TYPE ... AS ENUM (...)` types. This feature lets unit-only enums use
PostgreSQL native enums instead of integer discriminants.

## Scope

This applies to **unit-only enums** — enums where every variant has no fields.
Data-carrying enums still need nullable columns for variant fields, so they
continue to use integer discriminants regardless of database.

## Opting In

Add `#[toasty(pg_type = "type_name")]` to the enum definition:

```rust
#[derive(toasty::Embed)]
#[toasty(pg_type = "status")]
enum Status {
    #[column(variant = "pending")]
    Pending,
    #[column(variant = "active")]
    Active,
    #[column(variant = "done")]
    Done,
}
```

The `pg_type` attribute names the PostgreSQL enum type. Toasty generates:

```sql
CREATE TYPE status AS ENUM ('pending', 'active', 'done');
```

Variant labels come from the `#[column(variant = "...")]` attribute. When
`pg_type` is set, `variant` accepts a string instead of an integer.

## Variant Labels

Each variant must specify a string label with `#[column(variant = "label")]`:

```rust
#[derive(toasty::Embed)]
#[toasty(pg_type = "color")]
enum Color {
    #[column(variant = "red")]
    Red,
    #[column(variant = "green")]
    Green,
    #[column(variant = "blue")]
    Blue,
}
```

Labels are used as-is in the PostgreSQL enum type definition and in all queries.
They must be unique within the enum. Toasty emits a compile error if any labels
are duplicated or missing.

### Default Labels

If you omit the `variant` attribute on a variant, Toasty uses the variant name
converted to `snake_case`:

```rust
#[derive(toasty::Embed)]
#[toasty(pg_type = "color")]
enum Color {
    Red,        // → "red"
    Green,      // → "green"
    DarkBlue,   // → "dark_blue"
}
```

This is only available when `pg_type` is set. Without `pg_type`, every variant
still requires an explicit integer `#[column(variant = N)]`.

## Generated Schema

Given this model:

```rust
#[derive(toasty::Model)]
struct Task {
    #[key]
    #[auto]
    id: i64,
    status: Status,
}

#[derive(toasty::Embed)]
#[toasty(pg_type = "task_status")]
enum Status {
    #[column(variant = "pending")]
    Pending,
    #[column(variant = "in_progress")]
    InProgress,
    #[column(variant = "done")]
    Done,
}
```

Toasty generates this PostgreSQL schema:

```sql
CREATE TYPE task_status AS ENUM ('pending', 'in_progress', 'done');

CREATE TABLE tasks (
    id BIGSERIAL PRIMARY KEY,
    status task_status NOT NULL
);
```

The column type is the named enum type instead of `INT8`.

## Cross-Database Behavior

The `pg_type` attribute only affects PostgreSQL. On other databases, Toasty
falls back to integer discriminants. This means enums with `pg_type` must
provide **both** string labels and integer discriminants when targeting multiple
databases:

```rust
#[derive(toasty::Embed)]
#[toasty(pg_type = "task_status")]
enum Status {
    #[column(variant = 1, pg_label = "pending")]
    Pending,
    #[column(variant = 2, pg_label = "in_progress")]
    InProgress,
    #[column(variant = 3, pg_label = "done")]
    Done,
}
```

When both `variant` (integer) and `pg_label` are present:
- PostgreSQL uses `pg_label` as the enum label
- All other databases use `variant` as the integer discriminant

When `variant` is a string (as in the single-database examples above), it is
only valid for PostgreSQL. Toasty emits a compile error if a non-PostgreSQL
driver encounters string variant discriminants.

## Querying

Queries work the same as integer-discriminant enums. The generated query
builders are identical:

```rust
// Filter by variant
Task::all().filter(Task::FIELDS.status().is_pending())
Task::all().filter(Task::FIELDS.status().is_done())

// Equality
Task::all().filter(Task::FIELDS.status().eq(Status::InProgress))

// In-list
Task::all().filter(
    Task::FIELDS.status().in_list([Status::Pending, Status::InProgress])
)
```

Toasty handles the translation. On PostgreSQL, the generated SQL uses string
literals cast to the enum type:

```sql
SELECT * FROM tasks WHERE status = 'pending'::task_status;
SELECT * FROM tasks WHERE status IN ('pending'::task_status, 'in_progress'::task_status);
```

## Creating and Updating

No changes to the create or update API. Enum values are passed as Rust values:

```rust
// Create
Task::create().status(Status::Pending).exec(&db).await?;

// Update
task.update().status(Status::Done).exec(&db).await?;
```

## Enum Type in Column Customization

The `#[column(type = "...")]` attribute on the enum is not supported when
`pg_type` is set. The column type is determined by the PostgreSQL enum type name.
Toasty emits a compile error if both are present.

The `#[column("name")]` attribute on the model field still works for renaming
the column:

```rust
#[derive(toasty::Model)]
struct Task {
    #[key]
    #[auto]
    id: i64,
    #[column("task_status")]
    status: Status,
}
```

```sql
CREATE TABLE tasks (
    id BIGSERIAL PRIMARY KEY,
    task_status task_status NOT NULL
);
```

## Compile-Time Validation

Toasty checks the following at compile time:

| Rule | Error |
|---|---|
| `pg_type` on a data-carrying enum | "`pg_type` is only supported on unit enums" |
| Duplicate variant labels | "duplicate variant label `{label}`" |
| Missing variant label without default | "variant `{name}` requires `#[column(variant = N)]`" |
| String `variant` without `pg_type` | "string variant labels require `#[toasty(pg_type = \"...\")]`" |
| `pg_type` combined with `#[column(type = "...")]` | "`pg_type` and `#[column(type)]` cannot be used together" |
| Non-PostgreSQL driver with string-only variants | "string variant labels are only supported with PostgreSQL" |

## Examples

### Single-Database (PostgreSQL Only)

```rust
#[derive(toasty::Embed)]
#[toasty(pg_type = "role")]
enum Role {
    #[column(variant = "admin")]
    Admin,
    #[column(variant = "editor")]
    Editor,
    #[column(variant = "viewer")]
    Viewer,
}
```

### Single-Database with Default Labels

```rust
#[derive(toasty::Embed)]
#[toasty(pg_type = "role")]
enum Role {
    Admin,      // → "admin"
    Editor,     // → "editor"
    Viewer,     // → "viewer"
}
```

### Multi-Database

```rust
#[derive(toasty::Embed)]
#[toasty(pg_type = "role")]
enum Role {
    #[column(variant = 1, pg_label = "admin")]
    Admin,
    #[column(variant = 2, pg_label = "editor")]
    Editor,
    #[column(variant = 3, pg_label = "viewer")]
    Viewer,
}
```

### Full Model

```rust
#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i64,
    name: String,
    role: Role,
}

#[derive(toasty::Embed)]
#[toasty(pg_type = "user_role")]
enum Role {
    Admin,
    Editor,
    Viewer,
}

// Creates:
//   CREATE TYPE user_role AS ENUM ('admin', 'editor', 'viewer');
//   CREATE TABLE users (
//       id BIGSERIAL PRIMARY KEY,
//       name TEXT NOT NULL,
//       role user_role NOT NULL
//   );

// Usage is unchanged:
let admins = User::all()
    .filter(User::FIELDS.role().is_admin())
    .collect(&db)
    .await?;
```
