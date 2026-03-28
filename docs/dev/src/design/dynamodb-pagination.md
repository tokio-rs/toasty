# Engine-Aware Pagination

## Problem

Toasty's pagination builds cursors by extracting values from returned rows. `Paginate::exec()` fetches N+1 items, takes the order-by field values from the last row, and stores them as the cursor in `Page`. On the next page request, this cursor becomes an `Offset::After` that the engine converts into either a WHERE filter (SQL) or an `ExclusiveStartKey` (DynamoDB).

This breaks in two ways:

1. **DynamoDB: `LastEvaluatedKey` is not reconstructible from row data.** When querying a GSI, `LastEvaluatedKey` contains both the GSI key attributes *and* the table's primary key attributes. The current `ddb_key()` function only maps table primary key columns, so GSI pagination produces an incorrect `ExclusiveStartKey`. Additionally, DynamoDB's `Limit` caps items *evaluated*, not *returned* — with filters, a request for 11 items might return 7 with a `LastEvaluatedKey` indicating more data exists. The N+1 strategy misinterprets this as "no next page."

2. **SQL: ORDER BY columns may not be in the SELECT list.** Consider `SELECT name FROM users ORDER BY age, id`. The cursor needs `[age, id]` but neither is in the projection. `extract_cursor` reads from the returned row, so cursor construction fails when ORDER BY columns aren't projected.

Both problems have the same root cause: pagination cursor construction requires data that `Paginate::exec()` doesn't have access to. Only the engine sits between the user's projection and the database response, so **pagination must be an engine concern**.

## Current Flow

```
Paginate::exec()
  → sets limit to N+1
  → executor.exec(query) → engine pipeline → driver.exec(QueryPk{cursor, limit, ...})
  → DynamoDB driver: cursor → ExclusiveStartKey, limit → Limit
  → driver returns Response{rows: Stream(values)}   ← LastEvaluatedKey is discarded
  → Paginate collects all rows
  → if len > N: has_next=true, cursor = extract_cursor(last_row)
  → Page { items, next_cursor, prev_cursor }
```

`LastEvaluatedKey` from the DynamoDB response is discarded at `query_pk.rs:96-105` (only `res.items` is used). The engine doesn't know pagination is happening — `Paginate::exec()` just issues a normal query with a limit.

## Existing Engine Pattern

The engine already has the machinery needed. The back-ref projection flow (`plan/statement.rs:1287-1310`) loads extra columns and projects them away:

1. `load_data.columns` accumulates columns needed by child statements beyond what the user requested
2. The database returns all of them
3. `Project` nodes extract what's needed for internal use
4. The final projection strips the extras before returning to the user

Pagination uses the same pattern: during planning, add ORDER BY columns to `load_data.columns`, build an `extract_cursor` function targeting those column positions, and insert a `Project` to strip them from the output.

## Design

### Core Types

```rust
// Engine result type — replaces raw ValueStream return from engine::exec
pub struct ExecResponse {
    pub values: ValueStream,
    pub next_cursor: Option<stmt::Value>,
    pub prev_cursor: Option<stmt::Value>,
}

// Pagination config attached to MIR actions (ExecStatement, QueryPk)
pub struct PaginationConfig {
    pub page_size: i64,
    /// SQL: extract cursor values from loaded row (including non-projected ORDER BY columns).
    /// None for NoSQL drivers where the driver produces the cursor.
    pub extract_cursor: Option<eval::Func>,
}

// Added to driver operation::QueryPk
pub struct QueryPk {
    // ... existing fields ...
    pub pagination: Option<PaginationRequest>,
}

pub struct PaginationRequest {
    pub page_size: i64,
    pub cursor: Option<stmt::Value>,
}

// Added to driver Response
pub struct Response {
    pub rows: Rows,
    pub cursor: Option<stmt::Value>,
}
```

Cursors are `stmt::Value` everywhere. For SQL, the engine builds the cursor value by applying `extract_cursor` to the last row — the value is a record of the ORDER BY column values. For DynamoDB, the driver serializes `LastEvaluatedKey` into a `stmt::Value` (e.g., a record of the key attribute name/value pairs). The engine doesn't inspect the cursor's internal structure — it stores it, returns it in `ExecResponse`, and passes it back to the driver on the next page request.

### Backward Pagination and Capability

DynamoDB only supports forward pagination. Its `ExclusiveStartKey` / `LastEvaluatedKey` mechanism scans in one direction — there is no built-in "previous page" operation. SQL databases support backward pagination naturally by reversing the ORDER BY direction and the keyset comparison operators.

This is expressed as a driver capability:

```rust
pub struct Capability {
    // ... existing fields ...

    /// Whether the driver supports backward (previous-page) pagination.
    /// SQL: true. DynamoDB: false.
    pub backward_pagination: bool,
}
```

Set to `true` for all SQL drivers and `false` for `Capability::DYNAMODB`.

The planner uses this capability to decide:

- **If `backward_pagination` is true (SQL):** The engine can produce both `next_cursor` and `prev_cursor`. For a backward page request, it reverses ORDER BY, flips comparison operators in the WHERE clause, and reverses the result set before returning.
- **If `backward_pagination` is false (DynamoDB):** `ExecResponse::prev_cursor` is always `None`. `Page::prev()` returns an error (or `None`). The user-facing `Page` type can expose `has_prev()` so callers know whether backward navigation is available.

This keeps the pagination API uniform — `Page` always has `next_cursor` and `prev_cursor` fields — while letting drivers declare what they actually support. Application code can check `page.has_prev()` or `page.prev_cursor.is_some()` to decide whether to render a "Previous" button.

### SQL Path

During planning:

1. The planner detects pagination (a `Limit` with `Offset::After` or a new pagination marker in the statement AST).
2. It adds ORDER BY columns to `load_data.columns` if they aren't already present.
3. It builds `extract_cursor` as an `eval::Func` that projects the ORDER BY column positions from the loaded row. This uses `Expr::arg_project(0, [index])` — the same mechanism as back-ref projections.
4. It sets the SQL limit to `page_size + 1` for next-page detection.
5. It inserts a `Project` action after the query to strip the extra ORDER BY columns from the output.

During execution:

1. The executor runs the SQL query and collects rows.
2. If `len > page_size`: there's a next page. It applies `extract_cursor` to the last kept row (row at index `page_size - 1`) to produce the cursor `stmt::Value`, then truncates to `page_size`.
3. It stores the cursor in `ExecResponse::next_cursor`.

On resume (next page request):

1. `Page::next()` passes the cursor `stmt::Value` back into the engine.
2. The lower phase converts `Offset::After(cursor_value)` into WHERE filters, as it does today (`lower/paginate.rs`).

### DynamoDB Path

During planning:

1. The planner detects pagination.
2. It sets `pagination: Some(PaginationRequest { page_size, cursor })` on the `QueryPk` operation. The cursor is the `stmt::Value` from a previous page, or `None` for the first page.
3. No N+1. No `extract_cursor`. No extra columns.

During execution:

1. The executor sends `QueryPk` to the driver with the `PaginationRequest`.
2. The DynamoDB driver uses `page_size` as `Limit` and `cursor` as `ExclusiveStartKey` (deserializing the `stmt::Value` back to a `HashMap<String, AttributeValue>`).
3. The driver returns `Response { rows, cursor }` where `cursor` is the serialized `LastEvaluatedKey`, or `None` if there are no more results.
4. The executor reads `cursor` from the response and stores it in `ExecResponse::next_cursor`.

On resume:

1. `Page::next()` passes the cursor `stmt::Value` back into the engine.
2. The planner puts it into `PaginationRequest::cursor` on the `QueryPk` operation.
3. The DynamoDB driver deserializes it back to `ExclusiveStartKey`.

### VarStore Changes

`VarStore` stores `ExecResponse` instead of just `Rows`, so pagination metadata flows through the pipeline:

```rust
pub(crate) struct VarStore {
    slots: Vec<Option<Entry>>,
    tys: Vec<stmt::Type>,
}

struct Entry {
    response: ExecResponse,
    count: usize,
}
```

Only the final `returning` variable's cursor fields are propagated to the caller. Intermediate variables (inputs to `Project`, `NestedMerge`, etc.) carry `None` cursors.

### Page Changes

`Page` stores `Option<stmt::Value>` cursors (as it does today with `Option<stmt::Expr>`):

```rust
pub struct Page<M> {
    pub items: Vec<M>,
    query: Query<M>,
    pub next_cursor: Option<stmt::Value>,
    pub prev_cursor: Option<stmt::Value>,
}
```

`Paginate::exec()` becomes a thin wrapper:

```rust
pub async fn exec(self, executor: &mut dyn Executor) -> Result<Page<M::Output>> {
    let response = executor.exec_paginated(self.query).await?;
    let items = response.values.collect().await?
        .into_iter().map(M::load).collect::<Result<_>>()?;
    Ok(Page::new(items, query, response.next_cursor, response.prev_cursor))
}
```

## DynamoDB `stmt::Value` Cursor Format

The DynamoDB driver needs a convention for serializing `LastEvaluatedKey` into `stmt::Value`. A natural mapping:

```
LastEvaluatedKey: { "pk": S("abc"), "sk": N("42"), "gsi_pk": S("xyz") }
→ stmt::Value::Record([
    stmt::Value::String("pk"),    stmt::Value::String("abc"),
    stmt::Value::String("sk"),    stmt::Value::I64(42),
    stmt::Value::String("gsi_pk"), stmt::Value::String("xyz"),
  ])
```

Or more simply, a list of `(name, value)` pairs. The DynamoDB driver owns both serialization and deserialization, so the exact format is a driver-internal concern. The engine and `Page` treat it as an opaque `stmt::Value`.

## Implementation Plan

### Phase 1: ExecResponse infrastructure

Mechanical change. Add `ExecResponse` type, update `VarStore` to store it, update all action executors to wrap results with `cursor: None`. No behavioral changes.

### Phase 2: Planner pagination detection

Add `PaginationConfig` to `ExecStatement` and `QueryPk` MIR nodes. The planner populates it when it detects a paginated query. No execution changes yet.

### Phase 3: SQL pagination in the executor

The executor handles `PaginationConfig` on `ExecStatement`: applies N+1 detection, runs `extract_cursor` on the last row, stores cursor in `ExecResponse`. The planner adds ORDER BY columns to `load_data.columns` and inserts `Project` to strip them.

### Phase 4: DynamoDB pagination

Add `PaginationRequest` to `operation::QueryPk`. Add `cursor: Option<stmt::Value>` to `Response`. The DynamoDB driver serializes `LastEvaluatedKey` and returns it. The executor reads it and stores in `ExecResponse`.

### Phase 5: Simplify Paginate::exec

Move the pagination logic out of `Paginate::exec()`. It becomes a thin wrapper that calls the engine and reads `ExecResponse` metadata.
