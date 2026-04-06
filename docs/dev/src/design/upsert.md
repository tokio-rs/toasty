# Upsert

## Capabilities

- **Upsert (insert-or-update)**: atomically insert a record or update it if a
  matching record exists, based on a conflict target (unique column or
  composite key).
- **Conflict target**: specify which unique column(s) determine whether a
  record already exists.
- **Update control**: choose which fields to update on conflict — all
  non-key fields, a named subset, or expressions referencing the proposed
  values.
- **Insert-or-ignore**: on `create()`, skip the insert when a conflicting
  record exists instead of returning an error.
- **Bulk upsert**: upsert multiple records in a single operation.
- **Relationship-scoped upsert**: upsert within a has-many relationship scope.

## Upsert

### Basic usage

`upsert()` inserts a new record or updates an existing one. The conflict target
tells Toasty which unique field(s) to match on. If a record with that value
exists, the specified fields are updated. Otherwise, a new record is inserted.

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: Id<User>,

    #[unique]
    email: String,

    name: String,

    login_count: i64,
}
```

The simplest form sets all fields and updates all non-key fields on conflict:

```rust
let user = User::upsert()
    .on(User::EMAIL)
    .email("alice@example.com")
    .name("Alice")
    .login_count(1)
    .exec(&mut db)
    .await?;
```

If no record with `email = "alice@example.com"` exists, Toasty inserts one. If
a record exists, Toasty updates `name` and `login_count` to the given values.

### Choosing which fields to update

By default, every non-key field set on the builder is updated on conflict. To
update only specific fields, use `update()` with a closure:

```rust
let user = User::upsert()
    .on(User::EMAIL)
    .email("alice@example.com")
    .name("Alice")
    .login_count(1)
    .update(|u| {
        u.login_count(1)
    })
    .exec(&mut db)
    .await?;
```

This inserts the full record when new, but only updates `login_count` when the
record already exists. The `name` field is left unchanged on an existing record.

### Expressions in updates

The update closure accepts the same expressions as a normal update builder. To
increment a counter instead of replacing it:

```rust
let user = User::upsert()
    .on(User::EMAIL)
    .email("alice@example.com")
    .name("Alice")
    .login_count(1)
    .update(|u| {
        u.login_count(User::LOGIN_COUNT + 1)
    })
    .exec(&mut db)
    .await?;
```

On insert, `login_count` is set to `1`. On conflict, the existing
`login_count` is incremented by 1.

### Composite conflict targets

When uniqueness spans multiple columns, pass them as a tuple:

```rust
#[derive(Debug, toasty::Model)]
struct Membership {
    #[key]
    #[auto]
    id: Id<Membership>,

    #[unique(composite("membership_key"))]
    org_id: Id<Org>,

    #[unique(composite("membership_key"))]
    user_id: Id<User>,

    role: String,
}

let membership = Membership::upsert()
    .on((Membership::ORG_ID, Membership::USER_ID))
    .org_id(&org.id)
    .user_id(&user.id)
    .role("admin")
    .exec(&mut db)
    .await?;
```

### Return value

`upsert()` returns the record. Callers that need to know whether the record was
inserted or updated can use `exec_with_result()`:

```rust
let result = User::upsert()
    .on(User::EMAIL)
    .email("alice@example.com")
    .name("Alice")
    .login_count(1)
    .exec_with_result(&mut db)
    .await?;

if result.was_inserted() {
    println!("created new user: {:?}", result.into_inner());
} else {
    println!("updated existing user: {:?}", result.into_inner());
}
```

### Bulk upsert

To upsert multiple records in one operation:

```rust
User::upsert_many()
    .on(User::EMAIL)
    .item(User::upsert()
        .email("alice@example.com")
        .name("Alice")
        .login_count(1))
    .item(User::upsert()
        .email("bob@example.com")
        .name("Bob")
        .login_count(1))
    .exec(&mut db)
    .await?;
```

### Relationship-scoped upsert

Upsert works within relationship scopes, just like `create()`:

```rust
let todo = user.todos()
    .upsert()
    .on(Todo::TITLE)
    .title("Buy groceries")
    .completed(false)
    .exec(&mut db)
    .await?;
```

The `user_id` foreign key is set automatically from the scope.

## Insert-or-ignore

Insert-or-ignore lives on the `create()` builder. It inserts the record if no
conflict exists and silently does nothing if it does. The record is not updated.

### Basic usage

```rust
User::create()
    .email("alice@example.com")
    .name("Alice")
    .login_count(0)
    .on_conflict_ignore()
    .exec(&mut db)
    .await?;
```

If a user with this email exists, the insert is skipped. No error is raised and
no fields are modified.

### Batch insert-or-ignore

```rust
User::create_many()
    .item(User::create().email("alice@example.com").name("Alice").login_count(0))
    .item(User::create().email("bob@example.com").name("Bob").login_count(0))
    .on_conflict_ignore()
    .exec(&mut db)
    .await?;
```

Records that conflict with existing rows are skipped. Records that do not
conflict are inserted.

### Macro syntax

The `create!` macro supports insert-or-ignore with the `ignore_conflict` option:

```rust
toasty::create!(User {
    email: "alice@example.com",
    name: "Alice",
    login_count: 0,
} on_conflict: ignore).exec(&mut db).await?;
```
