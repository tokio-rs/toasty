# Optimistic Concurrency with `#[version]`

## Summary

A `#[version]` field attribute turns a `u64` model field into an
optimistic-concurrency counter that Toasty manages. Creating a record
initializes the counter to `1`. Updating or deleting a loaded instance
conditions the write on the counter's current value and increments it
atomically. A second writer that loaded the same record returns an error
instead of silently overwriting the first writer's changes. One attribute
covers DynamoDB, SQLite, PostgreSQL, and MySQL.

## Motivation

Two processes that load a record, change a field, and save back race each
other. Without coordination the second save wins and the first writer's
change is lost — the classic lost-update problem. Users who want safety
today write a version column by hand: adding a field, remembering to
increment it on every update, attaching a filter that compares the prior
value, and translating "zero rows affected" into a domain error. It is
tedious, easy to forget, and has to be repeated per model.

Toasty already carries a `Condition` on `stmt::Update` and `stmt::Delete`,
and the engine already compiles conditional updates against SQL backends
through the CTE and read-modify-write plans. The remaining gap is a
first-class declaration at the model level that wires up the same machinery
without user boilerplate.

## User-facing API

Mark a `u64` field with `#[version]`:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct Document {
    #[key]
    #[auto]
    id: uuid::Uuid,

    content: String,

    #[version]
    version: u64,
}
```

Each model allows at most one `#[version]` field. The field must be `u64`
and not nullable. Toasty owns the value — do not pass it to
`create()`, and do not set it through an update builder. Enforcement of
that ownership shares a decision with `#[auto]`, `#[default]`, and
`#[update]`; see [#748].

[#748]: https://github.com/tokio-rs/toasty/issues/748

### Create

A new record starts with `version == 1`:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: uuid::Uuid,
#     content: String,
#     #[version]
#     version: u64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let doc = toasty::create!(Document { content: "hello" })
    .exec(&mut db)
    .await?;

assert_eq!(doc.version, 1);
# Ok(())
# }
```

### Instance update

Calling `.update().exec()` on a loaded instance conditions the write on the
version the instance was loaded with and increments the stored value. After
`.exec()` returns, the in-memory instance reflects the new version:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: uuid::Uuid,
#     content: String,
#     #[version]
#     version: u64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let mut doc = toasty::create!(Document { content: "hello" })
    .exec(&mut db)
    .await?;
assert_eq!(doc.version, 1);

doc.update().content("world").exec(&mut db).await?;
assert_eq!(doc.version, 2);
# Ok(())
# }
```

If a concurrent writer has advanced the stored version since the instance
was loaded, `.exec()` returns a condition-failed error and the on-disk
record is left alone:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: uuid::Uuid,
#     content: String,
#     #[version]
#     version: u64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let mut a = toasty::create!(Document { content: "hello" })
    .exec(&mut db)
    .await?;
let mut b = Document::filter_by_id(a.id).get(&mut db).await?;

// `a` commits version 2.
a.update().content("updated").exec(&mut db).await?;

// `b` is stale — still holds version 1.
let result = b.update().content("conflict").exec(&mut db).await;
assert!(result.is_err());
# Ok(())
# }
```

The recovery pattern is to reload the record and retry:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: uuid::Uuid,
#     content: String,
#     #[version]
#     version: u64,
# }
# async fn __example(mut db: toasty::Db, id: uuid::Uuid) -> toasty::Result<()> {
loop {
    let mut doc = Document::filter_by_id(id).get(&mut db).await?;
    let new_content = format!("{} !", doc.content);

    match doc.update().content(new_content).exec(&mut db).await {
        Ok(()) => break,
        Err(e) if e.is_condition_failed() => continue,
        Err(e) => return Err(e),
    }
}
# Ok(())
# }
```

### Instance delete

`.delete().exec()` carries the same version condition. A stale handle
cannot delete a record that someone else has touched:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: uuid::Uuid,
#     content: String,
#     #[version]
#     version: u64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let mut doc = toasty::create!(Document { content: "hello" })
    .exec(&mut db)
    .await?;
let stale = Document::filter_by_id(doc.id).get(&mut db).await?;

doc.update().content("moved on").exec(&mut db).await?;

let result = stale.delete().exec(&mut db).await;
assert!(result.is_err());
# Ok(())
# }
```

### Query-based updates and deletes

Queries that run without a loaded instance — `Document::filter_by_id(id)
.update()...` and `Document::filter_by_id(id).delete()` — do not check
the caller's prior view of the row, because there is no "version I last
saw" to compare against. Use an instance when OCC matters.

A query-based update still advances the version counter. Every matched
row gets `version = version + 1` as part of the compiled statement, so a
concurrent instance holder sees a conflict on its next write. Without
this, an instance holder's `version == current` check could silently pass
against a row that a query-based writer has already modified.

A query-based delete does nothing special. The row is gone; there is no
counter to advance.

## Behavior

**Initial value.** On insert, a versioned field is initialized to `1`
regardless of whether the user supplied a value. The user cannot override
this, because the whole point of the attribute is for Toasty to own the
counter.

**Increment.** An instance update attaches two things to the compiled
statement: an assignment that sets the versioned column to `current + 1`,
and a condition that compares the versioned column to `current`. `current`
is the value the instance held at the moment `.update()` was called.

**Condition.** An instance delete attaches the same `column = current`
condition, with no assignment.

**Query-based increment.** A query-based update over a versioned model
carries a `version = version + 1` expression assignment — a column
reference, not a literal. Query-based writes never condition on the
version, but they always advance it so that any observable write moves
the counter forward. This preserves the invariant that a stale instance
holder eventually fails on its next update.

**Reload.** On a successful instance update the returned record carries
the new version (`current + 1`), so the same instance can be updated again
without reloading.

**Error type.** A version conflict surfaces as `Error::condition_failed`,
the same kind already raised by other conditional writes. Callers
distinguish it via `err.is_condition_failed()`.

**Row missing vs. version mismatch.** When an instance update or delete
fails because the row is no longer present at all, Toasty raises
`Error::record_not_found`. When the row is present but its version has
advanced, Toasty raises `Error::condition_failed`. Drivers make that
distinction — see "Driver integration" below.

**Batch create.** `create_many()` inserts every new record with
`version = 1`, treating the batch as N independent inserts from the
version's point of view.

**Interactions.**

- *Relations.* The versioned field is scalar, so it has no interaction
  with `BelongsTo`, `HasMany`, or `HasOne`. An update that touches a
  relation still goes through the instance's update path and still carries
  the version condition.
- *Unique indices.* On DynamoDB an update that changes a unique column
  performs surgery on the index table inside a transaction. The version
  condition joins the main-table update inside the same transaction, so a
  stale writer cannot race either the main row or its index entry.
- *Transactions.* When the user wraps their own transaction around several
  updates, each instance update is still a self-contained conditional
  write; the user's transaction boundary does not relax the per-row
  condition.

## Edge cases

**Field type.** The attribute rejects anything other than `u64` at macro
expansion time. `Option<u64>`, signed integers, and the `#[serialize]`
attribute are errors.

**Multiple versioned fields.** A model may declare at most one
`#[version]` field. A second declaration is a schema build error.

**Batch create with a duplicate primary key.** On DynamoDB, each insert
carries an `attribute_not_exists(<version column>)` guard so duplicate
primary keys inside a batch fail the whole batch. SQL backends fall back
to the database's primary-key uniqueness check, with the same end result
(insert rejected).

**Reading without writing.** Loading a row does not advance the version.
Readers are free to take snapshots without contending.

**Overflow.** `u64` comfortably exceeds any realistic lifetime update
rate; overflow is not handled.

**Wrapping a versioned instance in a stale reference.** Cloning a model
freezes a second handle at the current version. After one handle
successfully updates, the other is stale. This is the contract, not a
bug — the OCC guarantee is that a loaded snapshot only overwrites the row
it loaded.

## Driver integration

The feature is a schema annotation plus a statement-level condition. Most
of the machinery lives in the engine and is shared across drivers:

- `ModelRoot.version_field: Option<FieldId>` points at the versioned
  field.
- `Field.versionable` and `Column.versionable` flag the field and its
  underlying column.
- The macro's `update()` expansion emits the assignment and the condition.
- The macro's `delete()` expansion emits the condition.
- `stmt::Update.condition` and `stmt::Delete.condition` carry the
  expression through simplify, lower, plan, and exec.

### DynamoDB

The DynamoDB driver already consumes an update condition and attaches it
as `ConditionExpression` on `UpdateItem` / `TransactWriteItems`. For
`#[version]` no new capability flag is needed:

- *Single-key update* → `UpdateItem` with the version condition.
- *Multi-key update* → `TransactWriteItems` where each item carries the
  same condition.
- *Unique-index surgery* → the version condition joins the existing
  `attribute_not_exists` / `column = prev` condition with `AND`.
- *Delete* → `DeleteItem` with the version condition, threaded through
  `operation::DeleteByKey.condition`.

Version-mismatch versus row-missing is decided the same way DynamoDB
already decides it: when `ConditionalCheckFailedException` fires with
`ReturnValuesOnConditionCheckFailure::AllOld`, the driver inspects the
returned item. No old item → row missing → `record_not_found`. Old item
present → condition failed → `condition_failed`.

### SQL

SQL drivers consume the same `condition` through the existing conditional-
update plans. Two strategies, chosen per-backend based on capability:

1. **CTE plan** (PostgreSQL). Compiles to a single statement with two
   CTEs: a `SELECT` that counts matching rows and rows matching both the
   filter and the condition, followed by an `UPDATE` / `DELETE` whose
   filter is `original_filter AND (matched_count = conditioned_count)`.
   The planner inspects the CTE counts to decide between `Ok`,
   `record_not_found`, and `condition_failed`.

2. **Read-modify-write plan** (SQLite, MySQL). Opens a transaction, issues
   `SELECT … FOR UPDATE` (or the backend's best equivalent) returning the
   matched and conditioned counts, then issues the `UPDATE` / `DELETE`
   with the filter only if the counts permit. The engine raises the same
   three-way outcome from the counts.

The update path already goes through `plan_conditional_sql_query_as_cte`
and `plan_conditional_sql_query_as_rmw`. The delete path reuses the same
two planners — this is where the SQL work for `#[version]` concentrates,
since conditional delete is new.

The SQL serializer currently asserts `update.condition.is_none()` — a
leftover from before the conditional-update plans were introduced. Both
the update and delete serializers leave conditions to the planner, which
rewrites them into filter predicates before the statement reaches the
serializer. No SQL dialect receives a `CONDITION` clause.

### Expression-valued assignments

Instance updates carry a value assignment (`version = current + 1`
computed at compile time). Query-based updates carry an expression
assignment (`version = version + 1` referencing the column itself).
Both SQL and DynamoDB already accept expression-valued `SET` right-hand
sides — SQL through its native `UPDATE … SET col = col + 1` grammar,
DynamoDB through `UpdateExpression: "SET col = col + :one"`. No new
driver machinery is needed; the existing assignment compile path handles
both shapes.

### Driver capability

No new capability flag. Existing flags (`select_for_update`,
`cte_with_update`) already govern which conditional-update plan is used,
and the version feature piggybacks on that choice.

### Out-of-tree drivers

A driver that does not yet consume `stmt::Delete.condition` will not fail
to compile — the field defaults to empty. Such a driver silently ignores
the version check on delete until it wires the condition through. This is
intentional: the degradation is "no OCC on delete," which matches the
pre-feature behavior for that driver.

## Alternatives considered

**Timestamp column instead of a counter.** An `updated_at` column checked
and refreshed on every write would double as an audit trail. Rejected
because timestamp resolution is database-specific and two writes within
the same tick cannot be disambiguated. A monotonic integer is exact.

**Always-on OCC.** Version every model implicitly, without an attribute.
Rejected because not every model wants the read/write amplification of a
conditional write, and schema migrations would have to add a column to
every existing table.

**Builder-level opt-in at the call site.** Instead of an attribute, expose
`.update_if(|u| u.version_eq(v))` on the builder. Rejected because it
forces every caller to remember the pattern on every update; an attribute
is a schema fact, not a per-call decision.

**User-visible `version()` setter on the update builder.** Let callers
bump the counter manually. Rejected for the same reason as `#[auto]` on
keys: a value Toasty is responsible for should not be expressible in the
builder, because every caller exposing it invites mistakes.

## Open questions

- **Ownership enforcement.** The user-facing contract says Toasty owns
  the version value, but the generated builders currently expose a setter
  for it and the lowering only fills a default when the user left the
  field unset. The same gap affects `#[auto]`, `#[default]`, and
  `#[update]`. Tracked as a cross-cutting decision in [#748].
  *Blocking implementation — for `#[version]` specifically, user override
  on update corrupts the counter.*
- **Retry helper.** Should Toasty ship a `.retry_on_conflict(n)` combinator
  that reloads and re-applies the closure, or is that a userland pattern?
  *Deferrable.*
- **Query-based version bumping.** A query-based update over a specific
  primary key could legitimately check a version argument supplied by the
  caller. Current design says no — query-based writes are version-blind.
  *Deferrable.*
- **Composite conditions.** Users may want to layer their own
  `.update_if(...)` condition on top of a versioned update. The
  composition rule (AND) is obvious but the builder surface is not yet
  designed. *Blocking implementation of `update_if`.*

[#748]: https://github.com/tokio-rs/toasty/issues/748

## Out of scope

- **Versioning embedded enum variants.** `#[version]` on a field inside an
  embedded enum variant. The current design only accepts the attribute on
  root-model primitive fields.
- **Cross-model version fencing.** A single version value shared across
  multiple models (e.g. a parent and its children). Each model owns its
  own counter.
- **Automatic `updated_at` semantics.** Versioning does not imply a
  timestamp field; `#[auto]` on `updated_at` remains a separate feature.
- **Retry backoff.** The caller chooses whether and how to retry.
