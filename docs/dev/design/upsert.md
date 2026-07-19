# Upsert

## Summary

Toasty generates `upsert_by_*` builders for primary keys and unique
constraints. An upsert atomically creates a record when its conflict target is
absent or updates the matching record when it is present. Ordinary setters
apply to both branches. Shared mutations apply to a field's declared default
when creating a record and its stored value when updating one. On drivers that
support branch-specific assignments, `on_create` and `on_update` express
values that apply to only one branch.

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

### Applying shared mutations

A shared mutation reads the field's current value. Declare `#[default]` to
define that value when the upsert creates a record:

```rust
#[derive(toasty::Model)]
struct Counter {
    #[key]
    name: String,

    #[default(10)]
    count: i64,
}

let counter = Counter::upsert_by_name("requests")
    .count(toasty::stmt::subtract(3))
    .exec(&mut db)
    .await?;
```

The create branch stores seven. The update branch subtracts three from the
stored count. `add`, `increment`, `decrement`, `push`, `extend`, `pop`,
`remove`, and `remove_at` use the same rule. Backend capability checks still
apply; declaring a list default does not make `pop` available on a backend that
cannot update the last list element atomically.

A shared mutation on a field without `#[default]` returns
`invalid_statement`. A replacement assignment does not read the previous
value, so ordinary values, `stmt::set`, and `stmt::clear` do not require a
default. When only the update branch needs a mutation, use a complete
`on_create` value and put the mutation in `on_update`.

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
`EXCLUDED` name so the API does not expose SQL-specific terminology.
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
PostgreSQL and SQLite support these closures. The initial DynamoDB driver
reports `unsupported_feature` when either closure is present because
`UpdateItem` applies one update expression whether it creates or updates the
item.

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
| Ordinary replacement setter | Yes | Yes |
| Ordinary mutation | Apply to `#[default]` | Apply to stored value |
| `on_create` assignment | Yes | No |
| `on_update` assignment | No | Yes |
| `#[default]` | Yes | No |
| `#[update]` | Yes | Yes |

An `on_create` or `on_update` assignment overrides an earlier assignment to
the same field on that branch. Fields omitted from the builder use their
normal create behavior when inserting and remain unchanged when updating.
Conflict-target fields cannot be assigned through the returned builder.

Toasty evaluates a field's `#[default]` once while building the upsert. The
create branch applies the shared mutation to that value. An explicit
`on_create` assignment overrides the default and avoids evaluating it.

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
not lower an upsert into a read followed by a create or update. Each supporting
driver uses one atomic database operation.

An auto-generated primary key can still have an `upsert_by_<key>` method, but
the caller must supply the key value. The database cannot select a conflict on
a key value that it has not generated yet.

For PostgreSQL and SQLite, a proposed row can conflict with a unique constraint
other than the selected target. The statement fails with the normal constraint
error rather than updating a row selected by the other constraint.

The update branch never changes the selected conflict fields. This applies even
when an ordinary setter would otherwise assign the same proposed value.

## Driver integration

SQL drivers receive the upsert as `Operation::QuerySql` containing a lowered
`Insert` statement. Non-SQL drivers receive `Operation::Upsert` containing the
same create values, declared field defaults, selected conflict target,
assignments, ignore policy, and returning projection. A driver must execute the
operation atomically without selecting a branch in application code.

The capability contract describes which upsert forms a driver supports:

- exact unique targets or primary-key targets only;
- targeted insert-or-ignore;
- `on_create` and `on_update` assignments.

The verifier returns `unsupported_feature` before dispatch when the operation
exceeds the driver's capability. Assignment kinds on the update branch continue
to use the existing per-assignment capability flags.

PostgreSQL and SQLite serialize an upsert as `INSERT ... ON CONFLICT` with the
selected columns, `DO UPDATE SET` or `DO NOTHING`, and `RETURNING`. Both
backends support primary and unique conflict targets and distinct create and
update assignments.

MySQL's `ON DUPLICATE KEY UPDATE` does not select a conflict target. A proposed
row that conflicts with any primary key or unique index can update a row, and a
table with multiple unique indexes can match a different row than the named
Toasty target. The initial MySQL driver therefore reports `unsupported_feature`
before dispatch. Toasty does not use `INSERT IGNORE`, which suppresses errors
beyond the selected uniqueness conflict. A separate API for MySQL's
any-unique-key behavior can be designed later.

DynamoDB accepts only primary-key targets. The driver serializes a regular
upsert as one `UpdateItem` request without an item-existence condition.
DynamoDB applies the update expression to an existing item or creates a missing
item with the supplied key. The driver requests `ALL_NEW` to return the stored
model.

The initial driver supports assignments whose DynamoDB action has defined
behavior for both a present and missing item:

| Toasty assignment | DynamoDB update action |
|---|---|
| Replace with a value | `SET field = :value` |
| Replace with `None` | `REMOVE field` |
| Append | `SET field = list_append(if_not_exists(field, :default), :value)` |
| Add | `SET field = if_not_exists(field, :default) + :value` |
| Subtract | `SET field = if_not_exists(field, :default) - :value` |
| Required create default | `SET field = if_not_exists(field, :default)` |

A required create default is safe because every valid existing item already
contains the field. The verifier rejects a create default on a nullable field
and a field that combines `#[default]` with `#[update]`, because attribute
absence cannot distinguish a new item from an existing item with no value.

The initial DynamoDB driver reports `unsupported_feature` when an upsert uses
`on_create` or `on_update`. An `UpdateExpression` cannot generally select an
expression based on whether the item existed before the request;
`if_not_exists` tests the presence of one attribute instead.

`or_ignore` uses `PutItem` with `attribute_not_exists(pk)`. It uses
`TransactWriteItems` when the create must also populate Toasty-managed unique
secondary-index tables. A failed base-item condition maps to `None`; a conflict
on another unique constraint remains an error.

The initial DynamoDB driver also rejects a regular upsert that assigns a
Toasty-managed unique secondary-index field. Maintaining that index may require
different writes depending on whether the base item was created or updated,
which one `UpdateItem` cannot express. `or_ignore` does not have this
restriction because it only has a create branch.

SQL drivers continue to receive `Operation::QuerySql`; their SQL serializer
must support the upsert clause on `Insert`. A non-SQL driver that advertises
upsert support must handle `Operation::Upsert`. Other drivers fail verification
with `unsupported_feature` before dispatch.

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

### Infer mutation identity values

Toasty could treat missing numbers as zero and missing lists as empty. That
would make `subtract(3)` insert negative three and make `push(value)` insert a
single-element list, regardless of the model's create behavior. Requiring
`#[default]` uses the same initialization rule for create and upsert and also
supports nonzero numeric and nonempty list defaults.

### Infer the only unique constraint

`User::upsert()` could select a target when a model has one unique constraint.
Adding another constraint would then make existing code ambiguous or change
its meaning. Generated `upsert_by_*` methods keep the target explicit.

### Emulate branch-specific DynamoDB assignments

[`UpdateItem`] is DynamoDB's native upsert operation. It creates a missing item
or applies the same update expression to an existing item, which directly
implements ordinary shared assignments. `if_not_exists` can also initialize a
missing attribute.

It cannot implement arbitrary `on_create` and `on_update` closures because
`if_not_exists` tests whether one attribute is present, not whether the item
existed before the request. Toasty could emulate the two branches with
conditional retries, but that adds multiple requests and retry behavior to an
operation DynamoDB otherwise handles with one request. The initial driver
instead reports `unsupported_feature` for the branch-specific API.

[`UpdateItem`]: https://docs.aws.amazon.com/amazondynamodb/latest/APIReference/API_UpdateItem.html

### Read, then create or update

A check followed by a write is not atomic. Supporting drivers use one native
database operation instead. Backends that cannot express the requested upsert
report `unsupported_feature`.

## Open questions

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
- DynamoDB `on_create` and `on_update` support is deferred because its native
  upsert applies one update expression to new and existing items.
- Assigning a DynamoDB unique secondary-index field during a regular upsert is
  deferred because maintaining its index requires branch-specific writes.
- MongoDB query-filter upsert is outside the current driver set.
