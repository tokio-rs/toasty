# DynamoDB Scan Support

## Summary

Toasty queries with no partition key predicate on any available index are given
a user-facing error today. On DynamoDB, these queries should execute as a table
`Scan`. This change adds `Operation::Scan` and wires it into the planner, exec
layer, and DynamoDB driver so that `User::all()` and non-key filters work out of
the box.

## Motivation

DynamoDB users cannot currently retrieve all records of a model or filter on
non-key attributes without knowing the partition key in advance. The following
queries fail at runtime with an unsupported-feature error:

```rust
// Fetch every user in the table
let users = User::all().exec(&mut db).await?;

// Filter on a non-key attribute
let active = User::filter(User::fields().active().eq(true))
    .exec(&mut db)
    .await?;
```

Both queries are valid in Toasty's API and work on SQL backends. The missing
piece on DynamoDB is a Scan fallback when the planner cannot find a viable index.

## User-facing API

No new methods are added. Queries that previously failed now succeed.

### Fetching all records

`User::all()` works on DynamoDB as on SQL backends:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     id: String,
#     name: String,
#     active: bool,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let users = User::all().exec(&mut db).await?;
# Ok(())
# }
```

### Filtering on non-key attributes

Filters that do not include the partition key fall through to a Scan with a
DynamoDB `FilterExpression`:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     id: String,
#     name: String,
#     active: bool,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let active_users = User::filter(User::fields().active().eq(true))
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

### Limiting results

`.limit()` returns exactly that many items. Because DynamoDB applies its `Limit`
parameter before evaluating `FilterExpression`, a single Scan call may return
fewer items than requested even when more matching items exist. The exec layer
loops on `LastEvaluatedKey`, accumulating results, until either `limit` items
have been collected or the table is exhausted:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     id: String,
#     name: String,
#     active: bool,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let sample = User::all().limit(10).exec(&mut db).await?;
# Ok(())
# }
```

### Pagination

`.paginate()` returns a page of results and a cursor for the next page:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     id: String,
#     name: String,
#     active: bool,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let page = User::all().paginate(25).exec(&mut db).await?;

for user in &page.items {
    println!("{}", user.name);
}

if let Some(cursor) = page.next_cursor {
    let next_page = User::all().paginate(25).after(cursor).exec(&mut db).await?;
}
# Ok(())
# }
```

## Behavior

**Happy path — no limit.** The exec layer issues repeated DynamoDB `Scan`
calls, following `LastEvaluatedKey` until the response has no continuation
token, then returns all collected rows.

**Happy path — limit set.** The exec layer issues repeated Scan calls following
`LastEvaluatedKey`, accumulating results until `limit` items have been collected
or the table is exhausted. DynamoDB applies its `Limit` parameter before
evaluating `FilterExpression`, so a single call may return fewer items than
requested even when more matching items exist; looping compensates for this.

**Pagination.** Each `.paginate()` call maps to a single Scan call. DynamoDB's
`Limit` is set to the page size and the call returns immediately. The response
cursor (`LastEvaluatedKey`) is threaded back to the caller as-is — no
compensating loop is needed because the caller drives paging explicitly.

**Filters.** Non-key filter expressions are sent as DynamoDB `FilterExpression`.
DynamoDB evaluates these server-side after reading each page, so the number of
items returned per call can be less than `Limit`.

**Error — ordering.** A query with `.order_by()` that has no viable index and
would resolve to a Scan returns a user-facing error from the planner:

```
DynamoDB Scan does not support ordering. Add an index on the fields you are
filtering by, or remove the order_by clause.
```

Paginated scans cannot support ordering because the full table is never in
memory at once. For limited scans, callers can sort the returned `Vec`
themselves.

## Edge cases

- DynamoDB's `Limit` parameter limits items *scanned*, not items returned.
  After server-side filtering, a single call may return fewer items than
  requested. The exec layer compensates by looping until `limit` items are
  collected or the table is exhausted — except during `.paginate()`, where the
  caller drives paging and a single call is always issued.

- `LastEvaluatedKey` may be present even when the returned item count equals
  the page size. The cursor is passed back to the caller as-is; a subsequent
  page call may return zero items.

- An empty table returns an empty `Vec` with no error.

## Driver integration

### New `Operation::Scan` variant

Driver implementors must handle a new variant in the `Operation` enum:

```rust
pub struct Scan {
    pub table: TableId,
    pub select: Vec<ColumnId>,
    pub filter: Option<stmt::Expr>,
    pub limit: Option<i64>,
    pub cursor: Option<stmt::Value>,
}
```

`Operation::Scan` is only emitted when the driver capability `sql = false`. SQL
drivers do not need to handle it. Out-of-tree drivers that set `sql = false`
must add a match arm for `Operation::Scan` in their `Connection::exec`
implementation, or return `Error::unsupported_feature`.

### DynamoDB implementation

- Map `filter` to a `FilterExpression` with `ExpressionAttributeNames` /
  `ExpressionAttributeValues` using the existing expression-serialization
  helpers.
- Map `limit` to the DynamoDB `Limit` parameter.
- Map `cursor` to `ExclusiveStartKey`.
- Return `LastEvaluatedKey` (if present) as `ExecResponse::next_cursor`.

## Alternatives considered

**Extend `QueryPk` with an optional key condition.** Rejected. `QueryPk` always
carries a key condition expression; making it optional blurs the semantic
boundary between key-condition queries and full scans and would require all
driver implementations to handle the nil-key case.

**Select the "cheapest" GSI for the scan.** Rejected. Toasty has no cardinality
information and no way for users to specify an index preference — index
selection is entirely implicit. Silently choosing a GSI could scan more data
than the base table if the GSI is sparse. Base table scan is always correct.

## Open questions

None blocking acceptance.

## Out of scope

**GSI scans.** Toasty currently has no mechanism for directing an operation to a
specific index — index selection is implicit. GSI scan selection has no viable
heuristic without cardinality information.

**Ordering on scans.** Not supported. See Behavior § "Error — ordering".
