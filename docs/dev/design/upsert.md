# Upsert

## Overview

Upsert is an atomic insert-or-update operation. Given a record and a conflict
target, it inserts the record if no matching record exists, or updates the
existing record if one does. The operation executes as a single atomic
statement — there is no window where a concurrent writer can interleave between
a check and a write.

## Capabilities

**Core:**

- Atomic insert-or-update keyed on a conflict target (unique column or
  composite key).
- Control over which fields are updated on conflict: all fields, a subset, or
  expressions that reference the existing row.
- Insert-or-ignore: insert the record or silently do nothing on conflict.
- Bulk upsert: upsert multiple records in a single operation.

**SQL backends (PostgreSQL, SQLite, MySQL):**

- `INSERT ... ON CONFLICT (col) DO UPDATE SET ...` (PostgreSQL, SQLite).
- `INSERT ... ON DUPLICATE KEY UPDATE ...` (MySQL).
- Conflict target can be any unique column(s) or constraint, not just the
  primary key.
- The `EXCLUDED` pseudo-table lets the update clause reference proposed values
  from the `VALUES` row.
- Conditional update: a `WHERE` clause on the `DO UPDATE` can skip the update
  for rows that don't meet a predicate.
- Bulk upsert maps to a single multi-row `INSERT ... VALUES` statement.

**DynamoDB:**

- `PutItem`: unconditional full-item replace keyed on the primary key. Inserts
  a new item or replaces all attributes of an existing item.
- `UpdateItem`: partial upsert keyed on the primary key. Updates specified
  attributes if the item exists, creates a new item with those attributes if it
  doesn't. Supports atomic expressions: `SET login_count =
  if_not_exists(login_count, 0) + 1`.
- Conflict target is always the primary key — DynamoDB does not support
  conflict on arbitrary unique columns.
- No bulk `UpdateItem`. `BatchWriteItem` supports `PutItem` only.
- `ConditionExpression` can gate the entire operation (both the insert and
  update paths), unlike SQL where `WHERE` only gates the update.

**MongoDB:**

- `updateOne` with `upsert: true`: if the filter matches a document, update
  it. If no document matches, create one from the filter fields merged with
  the update fields.
- Conflict target is the query filter, which can match on any field — not
  limited to unique fields or the primary key.
- Rich atomic update operators: `$set`, `$inc`, `$push`, `$addToSet`, `$min`,
  `$max`.
- `$setOnInsert` sets fields only when inserting, not when updating. This is a
  native primitive for the divergent pattern.
- Bulk upsert via `bulkWrite` with `updateOne` operations.

## Upsert patterns

Upsert use cases fall into four patterns based on how the update path relates
to the insert path.

### Replace

Insert the record if new. If it exists, replace all fields with the proposed
values. The update is identical to the insert — no field needs special
handling.

```
UPSERT INTO users (email, name, login_count)
VALUES ('alice@example.com', 'Alice', 1)
ON CONFLICT (email)
DO UPDATE SET name = EXCLUDED.name,
              login_count = EXCLUDED.login_count
```

DynamoDB equivalent: `PutItem` — writes the full item unconditionally.
MongoDB equivalent: `updateOne({ email }, { $set: { name, login_count } },
{ upsert: true })`.

This is the most common pattern. Syncing data from an external source,
idempotent event handlers, and cache warming all follow this shape.

### Replace with field tweaks

Insert the record if new. If it exists, replace most fields with the proposed
values, but apply expressions to a small number of fields that reference the
existing row.

```
UPSERT INTO users (email, name, login_count)
VALUES ('alice@example.com', 'Alice', 1)
ON CONFLICT (email)
DO UPDATE SET name = EXCLUDED.name,
              login_count = users.login_count + 1
```

DynamoDB equivalent: `UpdateItem` with a mix of `SET name = :name` and
`SET login_count = if_not_exists(login_count, :zero) + :one`.
MongoDB equivalent: `updateOne({ email }, { $set: { name }, $inc:
{ login_count: 1 } }, { upsert: true })`.

The typical case is a counter or timestamp: insert with an initial value,
increment or refresh on subsequent writes. Most fields are still plain
replacement; only a few need the existing value.

### Divergent insert and update

Insert and update set mostly different fields. The insert path populates
fields like `created_at` or `status` with initial values, while the update
path touches a different set of fields and ignores the initial values entirely.

```
UPSERT INTO users (email, name, login_count, status)
VALUES ('alice@example.com', 'Alice', 0, 'active')
ON CONFLICT (email)
DO UPDATE SET login_count = users.login_count + 1
-- name and status are NOT updated
```

On insert: all four columns are written. On conflict: only `login_count`
changes. `name` and `status` keep their existing values.

MongoDB handles this natively with `$setOnInsert`:
`updateOne({ email }, { $setOnInsert: { name: "Alice", status: "active" },
$inc: { login_count: 1 } }, { upsert: true })`. Fields in `$setOnInsert`
are written only on insert. SQL and DynamoDB have no equivalent — the insert
and update column lists must be constructed to achieve the same effect.

### Conditional upsert

Insert the record if new. If it exists, only update when a predicate on the
existing row holds. Rows that fail the predicate are left unchanged — the
operation silently skips them.

```
UPSERT INTO users (email, name, role)
VALUES ('alice@example.com', 'Alice', 'member')
ON CONFLICT (email)
DO UPDATE SET name = EXCLUDED.name
WHERE users.role != 'admin'
```

This protects certain rows from being overwritten. The insert always happens
(there is no existing row to check), but the update path is gated.

DynamoDB equivalent: `ConditionExpression` on `UpdateItem`. One difference:
DynamoDB's condition gates *both* the insert and update paths, so it can also
prevent inserts. SQL's `WHERE` on `DO UPDATE` only gates the update.

### Insert-or-ignore

A degenerate case: insert the record if new, do nothing if it exists. No
fields are updated. This is not an upsert in the traditional sense, but it
shares the same conflict-detection mechanism.

```
INSERT INTO users (email, name)
VALUES ('alice@example.com', 'Alice')
ON CONFLICT (email) DO NOTHING
```

DynamoDB equivalent: `PutItem` with
`ConditionExpression: attribute_not_exists(pk)`.

## Bulk upsert

All patterns above extend to multiple rows. SQL handles this with a single
multi-row `VALUES` clause:

```
UPSERT INTO users (email, name, login_count) VALUES
  ('alice@example.com', 'Alice', 1),
  ('bob@example.com', 'Bob', 1),
  ('carol@example.com', 'Carol', 1)
ON CONFLICT (email)
DO UPDATE SET name = EXCLUDED.name,
              login_count = users.login_count + 1
```

Each conflicting row is updated using its own proposed values via `EXCLUDED`.
The update clause is shared across all rows — per-row update logic is not
possible in a single SQL statement.

DynamoDB does not support bulk `UpdateItem`. A bulk upsert against DynamoDB
requires issuing one `UpdateItem` per item. `BatchWriteItem` supports
`PutItem` (replace pattern) but not `UpdateItem`.

MongoDB supports bulk upsert via `bulkWrite` with individual `updateOne`
operations, each with `upsert: true`. Unlike SQL, each operation in the batch
can have its own filter and update logic.

## Conflict target

The conflict target identifies which unique constraint determines whether a
record "already exists."

**SQL:** The conflict target can be any unique column, composite unique
columns, or a named constraint. A table may have multiple unique constraints,
so the conflict target must be specified explicitly.

```
-- Single column
ON CONFLICT (email)

-- Composite
ON CONFLICT (org_id, user_id)

-- Named constraint (PostgreSQL)
ON CONFLICT ON CONSTRAINT users_email_key
```

**DynamoDB:** The conflict target is always the primary key (partition key, or
partition key + sort key). There is no way to upsert on an arbitrary unique
attribute — DynamoDB does not enforce uniqueness on non-key attributes.

**MongoDB:** The conflict target is a query filter passed to `updateOne`. This
can match on any field, not just unique ones. If no document matches, MongoDB
creates one by merging the filter fields with the update. MongoDB also provides
`$setOnInsert` — a native way to specify fields that are written only on
insert, not on update — which maps directly to the divergent pattern without
requiring the update clause to explicitly omit fields.
