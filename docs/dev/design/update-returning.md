# Returning Models from Updates

## Summary

Query updates keep returning `()`. `.affected_count()` returns the backend's
affected count. `.all()`, `.first()`, and `.one()` update every matching row but
control how many updated models are returned. They can select old or new values.

## Motivation

`()` is the only portable default. Cassandra, for example, does not report
affected rows. Explicit counts avoid returning rows where supported.

Returning models avoids a second query, its extra round trip, and races with
other writers. Old values let callers remove replaced keys from caches or
search indexes.

## User-facing API

Executing a query update directly remains unchanged:

```rust
User::filter_by_active(false)
    .update()
    .active(true)
    .exec(&mut db)
    .await?;
```

Call `.affected_count()` after the assignments to request a count:

```rust
let count: u64 = User::filter_by_active(false)
    .update()
    .active(true)
    .affected_count()
    .exec(&mut db)
    .await?;
```

Call `.all()`, `.first()`, or `.one()` to return new models:

```rust
let users: Vec<User> = User::filter_by_active(false)
    .update()
    .active(true)
    .all()
    .exec(&mut db)
    .await?;
```

All three methods update every matching row. They control only return
cardinality: `.all()` returns every updated model, while `.first()` and `.one()`
return the first according to the query's ordering. `.first()` returns `None`
for no match; `.one()` returns a record-not-found error.

### Selecting old or new values

Model-returning statements default to new values. Call `.returning_old()` for
pre-update values or `.returning_new()` to select post-update values explicitly:

```rust
let previous: User = User::update_by_id(id)
    .email(new_email)
    .one()
    .returning_old()
    .exec(&mut db)
    .await?;

cache.remove(&previous.email).await?;
```

Both methods preserve the result cardinality and are available after `.all()`,
`.first()`, or `.one()`. All result forms compose with `toasty::batch()`.
Instance updates remain unchanged: they return `()` and reload the borrowed
model.

## Behavior

| Builder | Result | Default model version |
|---|---|---|
| `update.exec(&mut db)` | `Result<()>` | N/A |
| `update.affected_count().exec(&mut db)` | `Result<u64>` | N/A |
| `update.all().exec(&mut db)` | `Result<Vec<Model>>` | New |
| `update.first().exec(&mut db)` | `Result<Option<Model>>` | New |
| `update.one().exec(&mut db)` | `Result<Model>` | New |

Affected-count semantics follow the backend:

| Backend | Affected count |
|---|---|
| PostgreSQL | Updated rows, including unchanged values |
| SQLite, Turso | Directly updated rows; side effects excluded |
| MySQL | Matched rows, using `CLIENT_FOUND_ROWS` |
| DynamoDB | Successful root-item mutations, summed by Toasty |
| Cassandra-like drivers | `Error::unsupported_feature` |

Unsupported count requests fail before writing. Counts exclude changes caused
by relation updates.

Return cardinality never limits the update. `.first()` and `.one()` update every
match, then return the first model according to the query's ordering. Without
ordering, the backend may choose any updated model. `.one()` does not reject a
query that matches multiple rows; it differs from `.first()` only when no rows
match.

Returned models include no-op assignments. Deferred primitive and embedded
fields remain deferred, and relations remain unloaded.

| Backend | New values | Old values |
|---|---|---|
| PostgreSQL 18+ | Native | Native |
| PostgreSQL before 18 | Native | `Error::unsupported_feature` |
| SQLite, Turso | Native | `Error::unsupported_feature` |
| MySQL | `Error::unsupported_feature` | `Error::unsupported_feature` |
| DynamoDB `UpdateItem` | `ALL_NEW` | `ALL_OLD` |
| DynamoDB `TransactWriteItems` | `Error::unsupported_feature` | `Error::unsupported_feature` |

SQLite returns values from before subsequent `AFTER` triggers; PostgreSQL
returns values after update triggers. Toasty preserves these semantics.

DynamoDB updates that require `TransactWriteItems`, including changes to a
Toasty-managed unique-index field, cannot return models. Unsupported forms fail
before writing. In a batch, `.one()` errors use the existing rollback behavior.

## Edge cases

- `.all()` has no defined result order.
- No matches produce `()`, `0`, an empty vector, `None`, or a record-not-found
  error for the default, count, `.all()`, `.first()`, and `.one()` forms.
- Partial embedded and engine-managed assignments return complete models,
  subject to deferred fields.
- Relation assignments return only root models.
- Non-transactional DynamoDB multi-row updates retain existing partial-write
  behavior.

## Driver integration

Unit updates require no capability. Drivers report support for affected counts,
new values, and old values separately; unsupported requests fail before
writing.

Drivers apply the update to the full selection before narrowing returned rows
for `.first()` or `.one()`.

SQL drivers derive counts from statement-completion metadata. Key-value drivers
count successful root-item mutations without reading rows. SQL model returns
use projected `RETURNING` columns; old values require native pre-update row
references.

MySQL's non-atomic update-then-select fallback does not satisfy the model-return
contract. DynamoDB maps instance reloads, new models, and old models to
`UPDATED_NEW`, `ALL_NEW`, and `ALL_OLD`, and rejects model returns when planning
requires `TransactWriteItems`.

Out-of-tree drivers must preserve unit updates and existing instance reloads.
They may implement or reject affected counts and each model-returning mode.

## Alternatives considered

### Return new models by default

This transfers and decodes unused rows and makes ordinary updates unsupported
where model returning is unavailable.

### Infer cardinality from `update_by_*`

Unique filters can match no row. Explicit `.first()` and `.one()` keep the
zero-row policy visible.

## Open questions

None.

## Out of scope

- Selected fields, eager-loaded relations, and deleted models.
- Separate matched-row and physically-changed-row metadata.
- Owned or borrowed return values from instance updates.
