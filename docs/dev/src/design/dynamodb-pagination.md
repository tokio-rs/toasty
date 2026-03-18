# DynamoDB Pagination

## Problem

Toasty's pagination builds cursors by extracting values from returned rows. `Paginate::exec()` fetches N+1 items, takes the order-by field values from the last row, and stores them as the cursor in `Page`. On the next page request, this cursor becomes an `Offset::After` that the engine converts into either a WHERE filter (SQL) or an `ExclusiveStartKey` (DynamoDB).

This breaks for DynamoDB because:

1. **`LastEvaluatedKey` is not reconstructible from row data.** When querying a GSI, DynamoDB's `LastEvaluatedKey` contains both the GSI key attributes *and* the table's primary key attributes. The current `ddb_key()` function only maps table primary key columns, so GSI pagination produces an incorrect `ExclusiveStartKey`.

2. **DynamoDB may return fewer items than `Limit`.** DynamoDB's `Limit` caps the number of items *evaluated*, not *returned*. With filters, a query requesting 11 items (N+1 strategy) might return 7 items with a `LastEvaluatedKey` indicating more data exists. The current code interprets `items.len() <= page_size` as "no next page," which is wrong.

3. **The cursor is opaque to the application.** `LastEvaluatedKey` is a DynamoDB-internal structure. It cannot be derived from the result set; only DynamoDB can produce it.

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

The `LastEvaluatedKey` from the DynamoDB response is never captured (`query_pk.rs:96-105` builds a `ValueStream` from `res.items` only).

## Approaches

### A: Augment `Response` with pagination metadata

Add cursor fields to the driver `Response` type so drivers can pass back pagination tokens alongside rows.

```rust
pub struct Response {
    pub rows: Rows,
    pub next_cursor: Option<stmt::Value>,  // new
    pub prev_cursor: Option<stmt::Value>,  // new
}
```

The DynamoDB driver would serialize `LastEvaluatedKey` into a `stmt::Value` and set `next_cursor`. The engine would propagate this through `VarStore` to the final result. `Paginate::exec()` would prefer the driver-provided cursor over extracting one from row data.

**Pros:**

- Small change surface. `Response` gains two fields, DynamoDB driver serializes `LastEvaluatedKey`, engine passes it through. SQL drivers set `None`.
- Useful beyond pagination. The same `Response` expansion could carry `rows_affected` counts for UPDATE/DELETE, making it a general-purpose metadata channel.
- No changes to the statement AST or engine planning phases. The cursor is a driver concern that flows through unchanged.
- Works correctly for GSI queries because the driver returns the actual `LastEvaluatedKey` from DynamoDB.

**Cons:**

- Leaks a per-database concept into the shared driver interface. `next_cursor` on `Response` is meaningless for SQL drivers, which handle pagination entirely through WHERE rewrites.
- `Paginate::exec()` now needs two code paths: one for drivers that return cursors, one for drivers that don't. This splits the pagination logic between the engine (SQL keyset filtering) and the driver (DynamoDB `LastEvaluatedKey`).
- Does not solve the N+1 detection problem. DynamoDB can return fewer items than requested due to its evaluation-based `Limit`. The engine still needs a way to know "there are more results" that isn't just `len > page_size`. This means `Response` probably also needs a `has_more: bool` or similar, adding more driver-specific surface.
- The cursor value is opaque bytes from DynamoDB's perspective but a `stmt::Value` in Toasty's type system. Serializing `LastEvaluatedKey` (a `HashMap<String, AttributeValue>`) into a `stmt::Value` requires a convention that both the driver's write path and read path agree on. This is doable but brittle.

### B: Engine-aware pagination with structured return values

The engine knows pagination is happening and structures the query so the driver returns cursors as part of the result set. The return value from the engine becomes something like `(Vec<Row>, prev_cursor, next_cursor)` — the cursors are first-class values in the engine's variable store, not metadata on `Response`.

For DynamoDB, the `QueryPk` operation would gain a `return_cursors: bool` flag. When set, the driver returns a structured value: the rows plus serialized `LastEvaluatedKey`. For SQL, the engine would ensure ORDER BY columns appear in the SELECT list, extract cursor values from the last row during execution, and strip the extra columns from the output via the existing `Project` mechanism.

```rust
// MIR action
pub struct QueryPk {
    // ... existing fields ...
    pub pagination: Option<PaginationConfig>,
}

pub struct PaginationConfig {
    pub page_size: i64,
    pub extract_cursor: Option<eval::Func>,  // SQL: extract from rows
    pub return_cursor_from_driver: bool,      // DynamoDB: driver returns it
}
```

The engine's executor would handle both paths: for SQL, it applies `extract_cursor` to the last row; for DynamoDB, it reads the cursor from the driver's structured return.

**Pros:**

- Pagination becomes an engine concern with a uniform output shape. `Page` always gets its cursors from the engine, regardless of backend.
- Handles the SQL case where ORDER BY columns aren't in the SELECT list. The engine adds them to the query and strips them in projection, which the existing `pagination.md` design doc already describes.
- Aligns with the existing `pagination.md` design (the `ExecResponse`/`Metadata` approach) and extends it for DynamoDB.
- The N+1 strategy can be replaced entirely for DynamoDB. Instead of fetching N+1 and checking length, the engine checks whether the driver returned a cursor. `has_next = cursor.is_some()`.

**Cons:**

- Larger change. Pagination awareness must flow through lower → plan → exec phases. The existing `pagination.md` design doc describes 5 phases of work.
- The `return_cursor_from_driver` flag means the engine still needs to know which kind of driver it's talking to, partially defeating the abstraction. The driver capability system (`capability()`) could gate this, but it's still a branch in the engine.
- The structured return from the driver (rows + cursor as a single `Value`) requires a convention. The engine and driver must agree on the shape, e.g., `Value::Record([Value::List(rows), cursor_value])`. This is a new pattern in the codebase.
- More complex to implement incrementally. The SQL path and DynamoDB path have different needs, and both must work before pagination is correct on either backend.

### C: Opaque cursor tokens at the driver boundary

Instead of representing cursors as `stmt::Value` (which implies the engine understands their structure), cursors become opaque byte blobs (`Vec<u8>`) that only the originating driver can interpret. The driver serializes its native pagination token (DynamoDB's `LastEvaluatedKey`, or SQL keyset values) into bytes, and the engine/`Page` store and pass them back without inspection.

```rust
// In driver
pub struct Response {
    pub rows: Rows,
    pub cursor_token: Option<CursorToken>,
}

pub struct CursorToken(pub Vec<u8>);

// In QueryPk operation
pub struct QueryPk {
    // ... existing fields ...
    pub cursor_token: Option<CursorToken>,  // replaces cursor: Option<stmt::Value>
}
```

For DynamoDB, `CursorToken` is a serialized `LastEvaluatedKey` (e.g., JSON or bincode of the `HashMap<String, AttributeValue>`). For SQL drivers, it's serialized keyset values. Each driver owns its serialization format.

`Page` stores `Option<CursorToken>` instead of `Option<stmt::Expr>`. The `Paginate` flow passes the token back to the engine, which passes it to the driver unchanged.

**Pros:**

- Clean abstraction boundary. The engine never inspects or constructs cursors. Drivers own the full lifecycle: produce token → store opaquely → receive token → resume query.
- Handles DynamoDB GSI pagination correctly by definition — the token is the actual `LastEvaluatedKey`.
- No `stmt::Value` serialization gymnastics. DynamoDB's `HashMap<String, AttributeValue>` maps naturally to bytes without fitting it into Toasty's value type system.
- SQL drivers can encode keyset values however they want, including columns not in the SELECT list (they'd query for extra columns and encode them into the token).
- The N+1 strategy moves into the driver for backends that need it (SQL) and is skipped for backends that don't (DynamoDB, which signals "more data" via `LastEvaluatedKey` presence).
- Works for future backends (e.g., Cassandra paging state tokens) without changes to the engine.

**Cons:**

- Breaks the current design where `Paginate` can manipulate cursor values as expressions. Today, `extract_cursor` evaluates order-by expressions against row data to produce a `Value` that becomes an `Offset::After(Expr)`. With opaque tokens, this manipulation moves into the driver.
- SQL drivers must now implement cursor extraction logic that currently lives in the engine (`lower/paginate.rs`). This duplicates the keyset pagination logic or requires the engine to delegate it. Either way, the SQL driver becomes more complex.
- Opaque tokens are harder to debug. A `stmt::Value` cursor is inspectable; a `Vec<u8>` requires driver-specific deserialization to examine.
- Cursor tokens are not stable across schema changes or driver versions. If the serialization format changes, outstanding tokens from previous pages become invalid. This is also true of the current approach but is more explicit with typed values.
- `Page::next()` and `Page::prev()` currently construct new `Paginate` queries using cursor expressions. With opaque tokens, the flow changes: `Page` must store enough context to reconstruct the query *and* pass the token, rather than embedding the cursor into the statement AST.

## Comparison

| Concern | A: Response metadata | B: Engine-aware | C: Opaque tokens |
|---|---|---|---|
| DynamoDB GSI correctness | Yes (driver returns real key) | Yes (driver returns real key) | Yes (by definition) |
| DynamoDB N+1 problem | Needs `has_more` flag too | Engine checks cursor presence | Driver signals via token presence |
| SQL ORDER BY not in SELECT | Not addressed | Handled (add to SELECT, strip in projection) | Driver handles |
| Change surface | Small (Response + DDB driver) | Large (lower/plan/exec + both drivers) | Medium (Response + both drivers + Page) |
| Abstraction leakage | Driver concept in shared interface | Engine branches on driver type | Clean boundary |
| Incremental delivery | Easy | Hard (both paths needed) | Medium |
| Debugging | Good (typed values) | Good (typed values) | Harder (opaque bytes) |
| Future backends | Needs per-backend fields or generic map | Needs per-backend branches in engine | Works naturally |

## Recommendation

My suggestion would be a hybrid of B and C. The key insight: the engine should own the *decision* of whether pagination is happening (it already does via `Paginate`), but the *cursor production* should be delegated to the driver via a well-defined protocol.

Concretely:

1. Add a `pagination: Option<PaginationRequest>` field to `QueryPk` and `QuerySql` operations, replacing the current `cursor` field and the separate limit/offset handling. `PaginationRequest` contains `page_size` and an optional `resume_token: Option<CursorToken>`.

2. Add `cursor_token: Option<CursorToken>` to `Response`. Drivers set this when there are more results.

3. For DynamoDB: the driver uses `page_size` as `Limit`, `resume_token` as `ExclusiveStartKey`, and returns `LastEvaluatedKey` as the `cursor_token`. No N+1 needed.

4. For SQL: the lower phase continues to rewrite `Offset::After` into WHERE filters. The SQL driver applies the N+1 strategy itself (request `page_size + 1`, check overflow, extract keyset values from the last row, encode as `CursorToken`). Or, the engine does N+1 and cursor extraction as it does today for SQL, and only uses the `cursor_token` path for KV drivers.

5. The engine propagates `cursor_token` through `VarStore` to the final result. `Page` stores `Option<CursorToken>` and passes it back on `.next()`/`.prev()`.

This gives DynamoDB correct pagination with minimal engine changes, keeps the SQL path close to what it is today, and uses opaque tokens at the boundary so future backends work without engine modifications.
