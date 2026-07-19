# Upserting Records

An upsert creates a record when its selected key is absent and updates the
matching record when that key is present. The database chooses the branch in
one atomic operation, so another writer cannot insert or update the record
between a preliminary read and the write.

## Creating or updating by a unique field

`#[derive(Model)]` generates `upsert_by_*` methods for primary keys and unique
constraints. Pass the conflict-target value to the method, set the remaining
fields, and call `.exec(&mut db)`:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     #[unique]
#     email: String,
#     name: String,
#     login_count: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let user = User::upsert_by_email("alice@example.com")
    .name("Alice")
    .login_count(1)
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

If the email is absent, Toasty creates a `User` with the supplied values. If
the email already exists, Toasty replaces that user's `name` and `login_count`.
The operation returns the record stored by the database in either case,
including generated fields such as `id`.

The conflict-target argument supplies the create value and never changes on
the update branch. The builder therefore has no `email` setter. Toasty does not
generate `upsert_by_*` for an ordinary `#[index]` because an indexed value can
match multiple records.

A composite unique constraint generates a method containing every target
field:

```rust,ignore
let membership = Membership::upsert_by_org_id_and_user_id(org_id, user_id)
    .role(Role::Admin)
    .exec(&mut db)
    .await?;
```

Toasty does not provide an unqualified `User::upsert()` method. Naming the
target prevents a new unique constraint from changing which conflict the
operation handles.

## Applying one assignment to both branches

An ordinary replacement setter supplies the value for both branches. A shared
mutation such as `increment`, `subtract`, or `push` reads the existing field
value on update. Add `#[default]` to define the value that the mutation reads
when Toasty creates the record:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Counter {
#     #[key]
#     name: String,
#     #[default(0)]
#     count: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let counter = Counter::upsert_by_name("requests")
    .count(toasty::stmt::increment())
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

This applies `increment` to the declared zero when `requests` is absent, so the
inserted count is one. On conflict, it atomically increments the stored count.
All shared mutations follow the same rule. For example, `#[default(10)]` with
`subtract(3)` inserts seven, and a list default with `pop()` inserts that list
without its last element.

A shared mutation on a field without `#[default]` returns
`invalid_statement`. Put a complete value in `on_create` and the mutation in
`on_update` when the model has no general default for that field. Replacement
assignments, including `set` and `clear`, do not read an existing value and do
not require `#[default]`.

## Setting different values on create and update

`on_create` adds assignments that run only when Toasty inserts a record.
`on_update` adds assignments that run only when the selected constraint
matches:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     #[unique]
#     email: String,
#     name: String,
#     login_count: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let user = User::upsert_by_email("alice@example.com")
    .on_create(|user| {
        user.name("Alice")
            .login_count(0)
    })
    .on_update(|user| {
        user.login_count(toasty::stmt::increment())
    })
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

The create branch stores the initial name and a zero count. The update branch
preserves the stored name and increments the stored count. A branch-specific
assignment overrides an ordinary setter for the same field on that branch.

The `on_update` builder accepts the same assignment operators as a normal
update. Its `incoming()` method references values proposed by the create
branch, while model field paths reference stored values:

```rust,ignore
User::upsert_by_email(email)
    .name(proposed_name)
    .login_count(1)
    .on_update(|user| {
        let incoming = user.incoming();
        user.name(incoming.name())
            .login_count(toasty::stmt::increment())
    })
    .exec(&mut db)
    .await?;
```

Ordinary setters already use the incoming value on update. Call `incoming()`
when an update expression needs to refer to a proposed field explicitly.

## Inserting or ignoring a conflict

Call `or_ignore()` when the selected conflict should leave the existing record
unchanged:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     #[unique]
#     email: String,
#     name: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let inserted: Option<User> = User::upsert_by_email("alice@example.com")
    .name("Alice")
    .or_ignore()
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

The result is `Some(user)` when Toasty creates the record and `None` when the
selected target conflicts. `or_ignore()` suppresses only that target's
conflict; a foreign-key failure, a missing required value, or a conflict on a
different unique constraint remains an error.

## Defaults and omitted fields

Upsert applies builder values and model attributes according to the selected
branch:

| Value source | Create branch | Update branch |
|---|---:|---:|
| Conflict-target argument | Applied | Unchanged |
| Ordinary replacement setter | Applied | Applied |
| Ordinary mutation | Applied to `#[default]` | Applied to stored value |
| `on_create` setter | Applied | Unchanged |
| `on_update` setter | Omitted | Applied |
| `#[default]` | Applied | Unchanged |
| `#[update]` | Applied | Applied |

Fields omitted from the builder use their normal create behavior when the row
is inserted and remain unchanged when an existing row is updated. The create
branch must still supply every required field that has no automatic value or
default.

A regular upsert must contain at least one update assignment. Call
`or_ignore()` when the conflict branch should make no changes; Toasty rejects
an empty update branch instead of generating a self-assignment that could run
database update behavior.

## Database support

Upsert support depends on whether a backend can identify the selected conflict
without changing Toasty's semantics:

| Backend | Primary key | Unique constraint | `on_create` / `on_update` | `or_ignore` |
|---|---:|---:|---:|---:|
| PostgreSQL | Yes | Yes | Yes | Yes |
| SQLite | Yes | Yes | Yes | Yes |
| Turso | Yes | Yes | Yes | Yes |
| DynamoDB | Yes | No | Required fields / No | Yes |
| MySQL | No | No | No | No |

DynamoDB executes a regular primary-key upsert with one `UpdateItem` request.
It lowers a create-only assignment on a required field to `if_not_exists`.
For a shared arithmetic or append mutation, the declared `#[default]` is the
`if_not_exists` operand, so DynamoDB uses the same initializer as SQL drivers.
Nullable create-only assignments and all update-only assignments return
`unsupported_feature`. DynamoDB also rejects regular upserts that assign a
Toasty-managed `#[unique]` field. Its `or_ignore()` form can initialize unique
fields because that form only has a create branch.

MySQL's `ON DUPLICATE KEY UPDATE` reacts to any unique conflict instead of the
target named by `upsert_by_*`. Toasty returns `unsupported_feature` rather than
updating a row selected by a different constraint.

Every supporting driver sends one atomic database operation. An upsert inside
a transaction also follows that transaction's isolation and commit behavior.

Upsert handles one record at a time and exposes setters for scalar and embedded
fields. It does not generate relation setters or nested-create builders.
