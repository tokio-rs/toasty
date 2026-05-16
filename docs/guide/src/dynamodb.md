# DynamoDB

Toasty's DynamoDB driver uses the official [`aws-sdk-dynamodb`] crate.
DynamoDB is a key-value / document store, not a SQL database, so the
mapping is meaningfully different from the SQL backends. Most Toasty
model code still works — [`create!`](./creating-records.md),
[`find_by_pk`](./querying-records.md),
[`filter_by_<indexed>`](./querying-records.md#filtering-by-indexed-fields),
[`#[unique]`](./indexes-and-unique-constraints.md#unique-fields),
[`#[version]`](./concurrency-control.md),
[`Vec<scalar>` fields](./field-options.md#scalar-arrays) — but the set
of queries you can write is narrower, and the chapter below catalogues
the gaps so you can avoid building a model that the driver cannot
serve.

[`aws-sdk-dynamodb`]: https://docs.rs/aws-sdk-dynamodb

## Enabling the driver

Add the `dynamodb` feature to Toasty in `Cargo.toml`:

```toml
[dependencies]
toasty = { version = "{{toasty_version}}", features = ["dynamodb"] }
```

Then pass a `dynamodb://` URL to `Db::builder`:

```rust,ignore
let db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .connect("dynamodb://us-east-1")
    .await?;
```

The URL is opaque to the driver — only the scheme matters. AWS region,
credentials, and the optional endpoint override come from the standard
AWS configuration sources: `AWS_REGION`, `AWS_ACCESS_KEY_ID` /
`AWS_SECRET_ACCESS_KEY`, the shared `~/.aws/credentials` file, IAM
instance profiles, and so on. The driver calls
`aws_config::defaults(BehaviorVersion::latest())` and uses whatever
that resolves.

To point at a local DynamoDB instance for development or tests, set
`AWS_ENDPOINT_URL_DYNAMODB`:

```bash
AWS_ENDPOINT_URL_DYNAMODB=http://localhost:8000 \
AWS_REGION=us-east-1 \
AWS_ACCESS_KEY_ID=dummy AWS_SECRET_ACCESS_KEY=dummy \
cargo run
```

The Toasty repository's `compose.yaml` boots `amazon/dynamodb-local` on
port 8000 for this purpose.

The connection pool described in [Database Setup](./database-setup.md)
does not apply: the AWS SDK manages its own HTTP connections internally,
so `max_pool_size` and the related knobs have no effect on this backend.

## Behavior specific to DynamoDB

Most of the Toasty surface works the same way on DynamoDB as on a SQL
backend: [define a model](./defining-models.md), derive `Model`, and
call [`create!`](./creating-records.md),
[`find_by_pk`](./querying-records.md),
[`filter_by_<field>`](./querying-records.md#filtering-by-indexed-fields),
[`update()`](./updating-records.md), and
[`delete()`](./deleting-records.md).
[Associations](./relationships.md),
[embedded structs and enums](./embedded-types.md),
[`#[unique]`](./indexes-and-unique-constraints.md#unique-fields), and
[`#[version]`](./concurrency-control.md) all work.

What's different falls into three buckets:

1. **No native types.** DynamoDB has three scalar attribute types:
   string (`S`), number (`N`), and binary (`B`). Everything else —
   UUIDs, timestamps, dates, decimals, enums — rides on top of those.
2. **A narrower set of supported queries.** No `LIKE`, no `!=` on the
   primary key, no backward pagination, no `ORDER BY` on a full-table
   scan.
3. **No interactive transactions and no migrations.** The driver
   creates tables on `push_schema` but does not generate or apply
   migrations, and `db.transaction()` returns an error.

The rest of this chapter walks through each of these.

## Type mapping

DynamoDB attributes are one of three scalar types or a List. Toasty
encodes Rust values into them as follows:

| Rust type | DynamoDB attribute |
|---|---|
| `bool` | `BOOL` |
| `i8`, `i16`, `i32`, `i64` | `N` (number, stringified on the wire) |
| `u8`, `u16`, `u32`, `u64` | `N` |
| `f32`, `f64` | `N` |
| `String` | `S` |
| `Vec<u8>` | `B` |
| `uuid::Uuid` | `S` (hyphenated form) |
| `rust_decimal::Decimal` *(feature)* | `S` |
| `jiff::Timestamp` *(feature)* | `S` (ISO 8601) |
| `jiff::civil::Date` / `Time` / `DateTime` *(feature)* | `S` |
| Embedded `enum` | `S` (variant tag) plus per-variant data attributes |
| `Vec<T>` *(T scalar)* | `L` (DynamoDB list of element attributes) |

### Notes on the type mapping

**Numbers ride as strings.** DynamoDB's `N` type is a decimal string on
the wire with up to 38 digits of precision. Integers and floats all
flow through this representation; Toasty parses them back into the Rust
type declared on the field. A `u64` field accepts the full unsigned
range — unlike the SQL backends, which cap unsigned integers at
`i64::MAX` because they ride a signed column.

**No native temporal types.** Timestamps, dates, times, and datetimes
all serialize to ISO 8601 strings. Sort order on a string-encoded
timestamp matches chronological order, so range queries on a sort key
work as expected — but the database has no awareness that the column is
a date.

**No native decimal type.** `rust_decimal::Decimal` round-trips through
a string. Comparison still works, but the database does not normalize
or round.

**No native enum type.** An `embed`-tagged enum stores the variant tag
in one attribute and the variant's data fields in further attributes on
the same item — there is no separate enum type object.

**`Vec<scalar>` lives on the `L` attribute.** A `tags: Vec<String>`
field maps to a single List attribute containing one `S` element per
tag. See [Field Options](./field-options.md) for the model-level
details.

## Keys and indexes

Every DynamoDB table has a primary key built from a **partition key**
and an optional **sort key** (called the "local key" in Toasty's macro
syntax). Use `#[key(partition = ..., local = ...)]` to map a model onto
that layout (see [Keys and Auto-Generation](./keys-and-auto-generation.md)):

```rust,ignore
#[derive(Debug, toasty::Model)]
#[key(partition = user_id, local = id)]
struct Post {
    user_id: uuid::Uuid,
    id: uuid::Uuid,
    title: String,
}
```

A single `#[key]` field on a regular type compiles to a partition-only
table.

### What works as a key type

The partition and sort key columns must be a string, a number, or a
binary blob — DynamoDB has no other key types. Toasty maps Rust types
onto those: `String` and `uuid::Uuid` become `S`, all integer and float
types become `N`, and `Vec<u8>` becomes `B`. Anything else as the key
type is a schema-build error.

### Auto-generation

`#[auto]` on a `uuid::Uuid` key works as on the SQL backends — it
generates a UUID v7 client-side. Auto-incrementing integer keys
(`#[auto]` on `i64`, `u64`, etc.) do **not** work on DynamoDB; the
database has no equivalent. Either pre-generate the key value yourself
or pick `uuid::Uuid` as the key type.

### Secondary indexes

A `#[index]` on a non-key field becomes a Global Secondary Index (GSI)
that projects all attributes. The GSI is built when `push_schema`
creates the table and can be queried through the usual
`filter_by_<field>` accessor.

### Uniqueness

`#[unique]` on a non-key field is enforced through a separate index
table — DynamoDB has no native unique constraint. For each unique
index Toasty creates a second table keyed on the unique value, and
inserts/updates that touch a unique field run through
`TransactWriteItems`: one `Put` against the main table plus one `Put`
against the index table guarded by `attribute_not_exists`. A duplicate
fails the conditional check and the entire transaction rolls back, so
the main table and the index table stay consistent.

Two operational consequences worth knowing:

- Every insert that touches a unique field issues two writes (one to
  the base table, one to each index table). The cost shows up in the
  AWS bill.
- Updates that *change* a unique value read the old value first, then
  delete the old index entry and insert the new one inside a single
  transaction. The transaction's condition expression catches
  concurrent writers atomically.

See [Indexes and Unique Constraints](./indexes-and-unique-constraints.md)
for the model-level syntax.

### Billing mode

Tables and GSIs are created with on-demand (PAY_PER_REQUEST) billing.
Toasty does not currently expose a knob to switch to provisioned
capacity — change that on the AWS console after the table exists if
your workload needs it.

## Supported queries

The query builder methods that translate cleanly to DynamoDB:

- [`find_by_pk(...)`](./querying-records.md) — single-item `GetItem`
  against the base table.
- [`filter_by_<partition>(...)` and
  `filter_by_<partition>_and_<local>(...)`](./querying-records.md#filtering-by-indexed-fields)
  — `Query` against the base table with the supplied values forming
  the key condition.
- `filter_by_<indexed_field>(...)` — `Query` against the GSI.
- `find_by_<unique_field>(...)` — `Query` against the unique index
  table to fetch the primary key, followed by `GetItem` on the base
  table.
- [`.limit(n)` and `.limit(n).offset(m)`](./sorting-limits-and-pagination.md#limiting-results)
  — server-side `Limit` plus cursor paging; `offset` is emulated by
  fetching `n + m` items and discarding the first `m`.
- [`.order_by(field.asc())` /
  `.order_by(field.desc())`](./sorting-limits-and-pagination.md#sorting-with-order_by)
  on a sort key or GSI sort key — `Query` flips `ScanIndexForward`.
- [`.select(...)`](./querying-records.md#projecting-columns-with-select)
  for column-narrow reads — projected to `ProjectionExpression` on the
  underlying operation.
- [`.starts_with("prefix")`](./filtering-with-expressions.md#starts_with)
  on a string column — native `begins_with()`.
- [`Vec<scalar>` predicates](./field-options.md#scalar-arrays):
  `.contains(v)`, `.len()`, `.is_empty()` lower to DynamoDB's
  `contains()` and `size()` functions.

[Cursor-based pagination](./sorting-limits-and-pagination.md#cursor-based-pagination)
(`.paginate(per_page).next()`) works and tracks DynamoDB's
`LastEvaluatedKey` under the hood. The 1 MB-per-response cap that
DynamoDB imposes on `Query` and `Scan` is transparent: the driver
follows `LastEvaluatedKey` automatically when the caller hasn't passed
a `limit`.

## Unsupported queries

These will either fail at planning time, fall back to a full-table
scan, or panic in the driver:

**`!=` on the primary key.** The query planner removes the predicate
from the index key condition and applies it as a post-filter, which
forces a `Scan` instead of a `Query`. Avoid `!=` against the partition
or sort key — it defeats the index.

**`LIKE` and `ILIKE`.** DynamoDB has no `LIKE` operator. Calling
[`.like(...)` or `.ilike(...)`](./filtering-with-expressions.md#string-pattern-matching)
on a string field panics inside the driver. Use `.starts_with(...)`
for prefix matching; substring and suffix matching are not supported
by the backend at all and have to be done client-side after fetching
the rows.

**Backward pagination.**
[`.paginate(per_page).prev(&db)`](./sorting-limits-and-pagination.md#navigating-pages)
does not work — the driver does not produce a `prev_cursor`. DynamoDB
itself can walk a query in either direction, but Toasty does not
currently generate the reverse cursor.

**`ORDER BY` on a full-table scan.** DynamoDB's `Scan` API returns
items in an arbitrary order with no server-side sort. A query that
needs both a scan *and* an ordering returns an unsupported-feature
error at planning time. Order is only available on a `Query`, which
means the partition key must be pinned.

**`is_superset` / `intersects` with a non-literal right-hand side.**
For [`Vec<scalar>` fields](./field-options.md#scalar-arrays) these
predicates expand to one `contains()` clause per element on the
right-hand side, so the right-hand side has to be a concrete `Vec<T>`
known at query-construction time. A column
reference or a subquery on the right is rejected with an
unsupported-feature error. On the SQL backends the same predicates
take any expression on the right.

```rust,ignore
// Works — concrete Vec on the right.
let admins = User::filter(User::fields().roles().is_superset(vec!["admin", "owner"]))
    .all(&mut db)
    .await?;
```

## Scans vs queries

A `Query` against DynamoDB is bounded by a partition key — it touches
only the items in one partition and is paid for by the bytes returned.
A `Scan` reads every item in the table and pays for every byte read.
Costs and latency between the two diverge sharply once a table grows.

The Toasty engine picks between them based on which fields the filter
touches:

- A filter that pins the partition key (and optionally constrains the
  sort key) compiles to a `Query`.
- A filter on a `#[index]` field compiles to a `Query` against that
  GSI.
- A filter on a `#[unique]` field compiles to a `Query` against the
  unique index table.
- A filter that pins no key — or one that uses `!=` on the partition
  key — falls back to a `Scan` with a `FilterExpression`.

Scan results come back unordered, so `.order_by(...)` combined with a
scan returns an error rather than silently re-sorting client-side. If
you need ordered access to every row, the model needs a partition key
you can pin (a synthetic "all rows" partition is a common pattern) and
a sort key to order on.

A `Scan` with no `limit` follows `LastEvaluatedKey` to drain every
page; a `Scan` with `.limit(n)` stops after the page that satisfies
the limit. Either way, full-table scans are expensive enough that the
driver does not emit them for queries that could match an index.

## Transactions and concurrency

**No interactive transactions.**
[`db.transaction()`](./transactions.md) returns an error on DynamoDB. The `Operation::Transaction` variant that the SQL drivers
handle is rejected with an unsupported-feature error before it reaches
the wire. DynamoDB *does* have `TransactWriteItems`, but it's a single
RPC that takes the full batch up front — not the begin/execute/commit
sequence the Toasty `Transaction` API exposes. Toasty uses
`TransactWriteItems` internally for unique-index maintenance and for
batched writes that need conditional checks, but it does not surface
as a transaction handle.

**No row-level locking.** DynamoDB has no equivalent of `SELECT ...
FOR UPDATE`. The engine relies on optimistic concurrency control
(`#[version]`) and on DynamoDB's per-item conditional expressions for
mutation-time safety.

**Optimistic concurrency works.** A `#[version]` field on a model
maps to a conditional `UpdateItem` (or a conditional `Put` inside a
transaction): the driver builds the right `attribute_not_exists` check
on insert and the right `version = :old` check on update. A concurrent
writer causes the conditional check to fail, and the engine surfaces
the failure as a serialization error so the caller can retry. See
[Concurrency Control](./concurrency-control.md) for the model-level
syntax.

**Unique-index updates are atomic with the main item.** Updating a
record's unique field issues a single `TransactWriteItems` that
covers the base-table update, the old index entry's delete, and the
new index entry's insert. Either everything commits or nothing does;
the index table will not drift out of sync with the main table.

## Migrations

The driver supports schema creation but does not generate migrations.
On first run, `push_schema` issues `CreateTable` for each model — base
tables plus one auxiliary table per unique index — with the GSIs and
attribute definitions filled in from the model. Tables are created
with on-demand billing.

`generate_migration` and `apply_migration` are not implemented and
will panic if called. The driver does not change column types, add
GSIs, or rename attributes on an existing table — DynamoDB itself
treats most of those as expensive or impossible operations, and Toasty
does not paper over the difference.

The practical upshot: model evolution on DynamoDB is a manual process
today. Adding a new attribute to a model is generally fine (DynamoDB
items are schemaless and new fields on new writes coexist with old
items that don't have them), but adding a GSI or a `#[unique]`
constraint to an existing model requires deleting and recreating the
table, or doing the schema change through the AWS console and
back-filling the index data yourself. The [Migrations and Schema
Management](./schema-management.md) chapter has more on what Toasty's
migration tooling does on the SQL backends — none of it currently
applies here.
