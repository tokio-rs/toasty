# Upsert

## Summary

Toasty generates `upsert_by_*` builders for primary keys and unique
constraints. An upsert atomically creates a record when its conflict target is
absent or updates the matching record when it is present. Ordinary setters
apply to both branches, while `on_create` and `on_update` express values that
apply to only one branch.

## Motivation

Applications often receive the same logical record more than once. External
data synchronization, idempotent event handlers, and counters all need to
create a missing record or update the existing record.

Today this requires a read followed by either a create or an update:

```rust
let user = User::filter_by_email(&email).first().exec(&mut db).await?;

match user {
    Some(mut user) => {
        user.update().name(&name).exec(&mut db).await?;
    }
    None => {
        User::create()
            .email(&email)
            .name(&name)
            .exec(&mut db)
            .await?;
    }
}
```

A concurrent writer can insert or update the record between the read and the
write. A transaction does not remove this race unless the application also
uses backend-specific isolation or locking. Toasty needs one operation whose
atomicity comes from the database's conflict handling.

## User-facing API

### Creating or updating by a unique field

Toasty generates `upsert_by_*` for primary keys and `#[unique]` constraints.
Pass the conflict-target value to the generated method, set the remaining
fields, and call `exec`:

```rust
let user = User::upsert_by_email("alice@example.com")
    .name("Alice")
    .exec(&mut db)
    .await?;
```

If no user has that email address, Toasty creates one. If a user already has
that email address, Toasty updates `name`. The operation returns the stored
`User` in either case.

The conflict target appears once. `upsert_by_email` supplies `email` to the
create branch and prevents the update branch from changing it. Toasty does not
generate `upsert_by_*` for ordinary `#[index]` fields because those fields can
match more than one record.

Composite unique constraints generate methods that follow the existing
`filter_by_*` and `update_by_*` naming convention:

```rust
let membership = Membership::upsert_by_org_id_and_user_id(org_id, user_id)
    .role(Role::Admin)
    .exec(&mut db)
    .await?;
```

Toasty does not infer a conflict target for `User::upsert()`. Naming the target
keeps the operation stable when a model gains another unique constraint.

### Using existing values on update

An `on_update` closure changes the conflict branch. It uses the same assignment
API as a normal update:

```rust
let user = User::upsert_by_email("alice@example.com")
    .name("Alice")
    .login_count(1)
    .on_update(|user| {
        user.login_count(toasty::stmt::increment())
    })
    .exec(&mut db)
    .await?;
```

The create branch stores `login_count = 1`. The update branch replaces `name`
and increments the stored `login_count`. An assignment in `on_update`
overrides an ordinary setter for the same field.

The update builder exposes `incoming()` for expressions that combine stored
and proposed values. Toasty uses `incoming` rather than PostgreSQL's
`EXCLUDED` name because the same expression can target non-SQL backends.
Ordinary setters create incoming-value assignments automatically; users reach
for `incoming()` only when an update expression combines stored and proposed
fields.

### Setting different values on create and update

Use `on_create` and `on_update` when the two branches set different fields:

```rust
let user = User::upsert_by_email("alice@example.com")
    .on_create(|user| {
        user.name("Alice")
            .login_count(0)
            .status(Status::Active)
    })
    .on_update(|user| {
        user.login_count(toasty::stmt::increment())
    })
    .exec(&mut db)
    .await?;
```

This creates a new user with the supplied initial values. On conflict, it only
increments `login_count`; it preserves the existing `name` and `status`.

Ordinary setters remain the concise form for values shared by both branches.
`on_create` and `on_update` are needed only for fields whose behavior differs.

### Inserting or ignoring

Call `or_ignore` when a conflict should leave the existing record unchanged:

```rust
let inserted: Option<User> = User::upsert_by_email("alice@example.com")
    .name("Alice")
    .or_ignore()
    .exec(&mut db)
    .await?;
```

The operation returns `Some(user)` when it creates the record and `None` when
the selected constraint conflicts. Toasty does not perform a no-op update to
return the existing record because that could run update triggers or change an
automatic update field.

## Behavior

`upsert_by_*` uses the following assignment rules:

| Value source | Create branch | Update branch |
|---|---:|---:|
| Conflict-target arguments | Yes | No |
| Ordinary field setter | Yes | Yes |
| `on_create` assignment | Yes | No |
| `on_update` assignment | No | Yes |
| `#[default]` | Yes | No |
| `#[update]` | Yes | Yes |

An `on_create` or `on_update` assignment overrides an earlier assignment to
the same field on that branch. Fields omitted from the builder use their
normal create behavior when inserting and remain unchanged when updating.
Conflict-target fields cannot be assigned through the returned builder.

The default `exec` returns the record stored by the database. Toasty does not
return the proposed create value because generated fields and update
expressions can make that value differ from the stored record. `or_ignore`
returns `Option<Model>` because the conflict branch does not produce a record.

An upsert with no update assignments is invalid. The user must either assign a
field on the update branch or call `or_ignore`. Toasty reports an
`invalid_statement` error instead of generating a self-assignment that could
run update behavior.

Constraint violations other than the selected conflict target remain errors.
`or_ignore` suppresses only a conflict on that target; it does not suppress
missing required values, foreign-key failures, type errors, or conflicts on a
different unique constraint.

Upsert supports scalar and embedded fields. Relation setters are not generated
for the initial upsert builder. An upsert executed in a transaction follows the
transaction's isolation and commit behavior, but the upsert itself remains one
atomic database operation.

## Edge cases

The database resolves concurrent upserts against the same target. Toasty does
not lower an upsert into a read followed by a create or update, even on a
backend that lacks native support, because that would violate the atomicity
contract.

An auto-generated primary key can still have an `upsert_by_<key>` method, but
the caller must supply the key value. The database cannot select a conflict on
a key value that it has not generated yet.

For PostgreSQL and SQLite, a proposed row can conflict with a unique constraint
other than the selected target. The statement fails with the normal constraint
error rather than updating a row selected by the other constraint.

The update branch never changes the selected conflict fields. This applies even
when an ordinary setter would otherwise assign the same proposed value.

## Driver integration

Drivers receive a new `Operation::Upsert` containing the create values, the
selected conflict target, update assignments, the ignore policy, and the
returning model projection. A driver must execute it as one atomic database
operation. It must not implement upsert by reading and then writing.

The capability contract describes which conflict targets and branch forms a
driver supports:

- exact unique targets or primary-key targets only;
- shared create/update assignments or distinct branch assignments;
- targeted insert-or-ignore.

The verifier returns `unsupported_feature` before dispatch when the operation
exceeds the driver's capability. This allows DynamoDB to accept primary-key
forms while rejecting non-primary-key targets or branch combinations it cannot
encode exactly.

PostgreSQL and SQLite serialize an upsert as `INSERT ... ON CONFLICT` with the
selected columns, `DO UPDATE SET` or `DO NOTHING`, and `RETURNING`. Both
backends support primary and unique conflict targets and distinct create and
update assignments.

MySQL's `ON DUPLICATE KEY UPDATE` does not select a conflict target. A proposed
row that conflicts with any primary key or unique index can update a row, and a
table with multiple unique indexes can match a different row than the named
Toasty target. The initial MySQL driver therefore reports `unsupported_feature`
for `Operation::Upsert`. Toasty does not use `INSERT IGNORE`, which suppresses
errors beyond the selected uniqueness conflict. A separate API for MySQL's
any-unique-key behavior can be designed later.

DynamoDB accepts only primary-key targets. Shared assignments map to one
`UpdateItem` with `ReturnValues=ALL_NEW`; insert-or-ignore maps to an
`UpdateItem` guarded by `attribute_not_exists` so a successful insert can also
return the stored item. Distinct `on_create` and `on_update` assignments are
accepted only when the driver can express their exact result in one DynamoDB
write. Other forms return `unsupported_feature` rather than approximating SQL
semantics.

Adding `Operation::Upsert` requires out-of-tree drivers to handle the new
variant. A driver may return `unsupported_feature` without implementing it.
Existing operations and their behavior do not change.

## Alternatives considered

### Decorate a create builder

The create-first form is concise:

```rust
User::create()
    .email(email)
    .name(name)
    .upsert_by_email()
```

It does not guarantee that the conflict field was set, and a completed create
builder has already combined explicit values, defaults, and omitted fields.
The generated constructor supplies the target once and retains the distinction
between create-only and shared assignments.

### Expose a SQL-style conflict builder

A general `on_conflict(User::fields().email())` API closely follows SQL, but it
requires users to configure a target and update policy for the common case. It
also exposes concepts such as `EXCLUDED` that do not map directly to DynamoDB.
The generated methods keep raw conflict configuration out of the primary API.

### Require separate create and update builders

An API with required `create` and `update` sections expresses every branch but
duplicates values in the common replacement case. Ordinary setters plus
optional `on_create` and `on_update` closures make shared values concise without
removing branch control.

### Infer the only unique constraint

`User::upsert()` could select a target when a model has one unique constraint.
Adding another constraint would then make existing code ambiguous or change
its meaning. Generated `upsert_by_*` methods keep the target explicit.

### Read, then create or update

This works on every backend but is not atomic. Toasty does not call a
multi-operation fallback an upsert.

## Open questions

- **Blocking implementation:** Define the exact DynamoDB assignment forms that
  one `UpdateItem` can implement while preserving the create and update branch
  results.
- **Deferrable:** Decide whether nullable unique fields receive generated
  `upsert_by_*` methods or remain unsupported because SQL unique constraints
  commonly permit multiple `NULL` values.

## Out of scope

- Bulk upsert is deferred because SQL uses one update policy for every row,
  while DynamoDB has no bulk `UpdateItem` operation.
- Conditional update predicates are deferred because SQL and DynamoDB gate
  different branches.
- Relation and nested-create upserts are deferred until their execution and
  return behavior can be specified without additional database operations.
- Reporting whether an upsert inserted or updated is deferred because backends
  do not expose that outcome consistently.
- A `toasty::upsert!` macro is deferred; the generated builder is already
  concise, and macro syntax can be added without changing its semantics.
- MySQL's any-unique-key update behavior is separate from targeted upsert.
- MongoDB query-filter upsert is outside the current driver set.
