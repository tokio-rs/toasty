# Returning Models from Updates

## Summary

Query-based updates gain `.all()`, `.first()`, and `.one()` modifiers that make
`exec` return stored models. The builders compose with `toasty::batch()`.
Backends without a native update-returning operation reject them before writing.

## Motivation

Query-based updates currently discard database-generated values. Reading them
requires a second query, which adds a round trip, can observe another writer's
changes, and may fail when the update changes a filtered field.

```rust
User::update_by_id(id)
    .login_count(toasty::stmt::increment())
    .exec(&mut db)
    .await?;

let user = User::filter_by_id(id).get(&mut db).await?;
```

Native mutation results return the values produced by the update itself,
including relative assignments, version fields, triggers, and generated values.

## User-facing API

Choose a result cardinality after setting the assignments:

```rust
let users: Vec<User> = User::filter(User::fields().active().eq(false))
    .update()
    .active(true)
    .all()
    .exec(&mut db)
    .await?;

let user: Option<User> = User::all()
    .order_by(User::fields().id().asc())
    .update()
    .active(true)
    .first()
    .exec(&mut db)
    .await?;

let user: User = User::update_by_id(id)
    .name("Alice Smith")
    .one()
    .exec(&mut db)
    .await?;
```

`.all()` updates every match. `.first()` and `.one()` update at most one row,
using the source query's ordering. `.first()` returns `None` for no match;
`.one()` returns the same record-not-found error as a one-record query.

Calling `exec` without a modifier remains a no-result update. Thus the earlier
read-after-write example becomes:

```rust
let user = User::update_by_id(id)
    .login_count(toasty::stmt::increment())
    .one()
    .exec(&mut db)
    .await?;
```

The modifiers produce typed statements for `toasty::batch()`:

```rust
let (users, post): (Vec<User>, Post) = toasty::batch((
    User::all().update().active(true).all(),
    Post::update_by_id(post_id).published(true).one(),
))
.exec(&mut db)
.await?;
```

Returning updates can be mixed with queries, creates, and no-result updates.

Instance updates keep returning `()` and reloading the borrowed model in place.
The modifiers apply only to query-based update builders.

## Behavior

| Builder | Result |
|---|---|
| `update.exec(&mut db)` | `Result<()>` |
| `update.all().exec(&mut db)` | `Result<Vec<Model>>` |
| `update.first().exec(&mut db)` | `Result<Option<Model>>` |
| `update.one().exec(&mut db)` | `Result<Model>` |

`.all()` preserves the full selection. `.first()` and `.one()` apply a one-row
limit before updating; without explicit ordering, the backend may select any
match. Like a one-record query, `.one()` does not test whether the unconstrained
selection would match multiple rows. Generated `update_by_*` builders do not
infer cardinality.

Each result contains the stored post-update state, not values reconstructed
from assignments. It includes rows even when an assignment changes a filtered
field or leaves all stored values unchanged. Normal field-loading rules apply:
deferred fields remain deferred, and relations remain unloaded.

Backend support is:

| Backend | Behavior |
|---|---|
| PostgreSQL, SQLite, Turso | Native `UPDATE ... RETURNING` |
| DynamoDB `UpdateItem` | `ReturnValues = ALL_NEW` |
| MySQL | `Error::unsupported_feature` before writing |
| DynamoDB `TransactWriteItems` | `Error::unsupported_feature` before writing |

DynamoDB uses its existing per-item behavior for multi-row updates. An update
that may require `TransactWriteItems`, including a change to a Toasty-managed
unique-index field, cannot return a model because transactions do not return
new items.

Existing assignment and condition errors remain unchanged. In a batch,
unsupported forms are rejected before any write; a `.one()` not-found error
uses the batch's existing rollback behavior.

## Edge cases

- `.all()` has no defined result order.
- Partial embedded-field and engine-managed assignments return their complete
  stored models, subject to deferred fields.
- Relation assignments return only the root models.
- A DynamoDB multi-row failure retains the driver's existing partial-write
  behavior for non-transactional updates.

## Driver integration

SQL drivers receive the existing update statement with a full-model returning
projection. For `.first()` and `.one()`, the engine first constrains the target
to the first key from the ordered source query. Drivers with
`Capability::returning_from_mutation == false` are rejected before execution.

Key-value drivers distinguish instance reloads from full-model results:

```rust
pub enum UpdateReturning {
    Changed(Vec<ColumnId>),
    Model(Vec<ColumnId>),
}
```

`UpdateByKey::returning` becomes `Option<UpdateReturning>`. `Changed` preserves
the existing sparse result used for instance reloads. `Model` returns every
requested column in order. DynamoDB maps them to `UPDATED_NEW` and `ALL_NEW`,
respectively, and rejects `Model` before any operation that may require
`TransactWriteItems`.

No new capability is needed. SQL uses `returning_from_mutation`; other drivers
reject unsupported operation forms before mutation. Changing the public
`UpdateByKey::returning` type is a breaking change for out-of-tree key-value
drivers, which must support `Changed` and either implement or reject `Model`.

The cardinality builders implement `IntoStatement` with `List<Model>`,
`Option<Model>`, or `Model` as their return type, enabling batch composition.

### Infer cardinality from `update_by_*`

Even unique filters can match no row. Explicit `.first()` and `.one()` keep the
zero-row policy visible and consistent across query-based updates.

### Return projected fields

Returning tuples requires a separate write-projection design. This proposal
returns models only.

## Open questions

None.

## Out of scope

- Selected fields, eager-loaded relations, and deleted models.
- Affected-row and matched-versus-changed metadata.
- Owned or borrowed return values from instance updates.
