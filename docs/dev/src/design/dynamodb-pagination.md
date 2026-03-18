# DynamoDB Pagination

## Problem

Toasty's pagination builds cursors by extracting values from returned rows. `Paginate::exec()` fetches N+1 items, takes the order-by field values from the last row, and stores them as the cursor in `Page`. On the next page request, this cursor becomes an `Offset::After` that the engine converts into either a WHERE filter (SQL) or an `ExclusiveStartKey` (DynamoDB).

This breaks for DynamoDB because:

1. **`LastEvaluatedKey` is not reconstructible from row data.** When querying a GSI, DynamoDB's `LastEvaluatedKey` contains both the GSI key attributes *and* the table's primary key attributes. The current `ddb_key()` function only maps table primary key columns, so GSI pagination produces an incorrect `ExclusiveStartKey`.

2. **DynamoDB may return fewer items than `Limit`.** DynamoDB's `Limit` caps the number of items *evaluated*, not *returned*. With filters, a query requesting 11 items (N+1 strategy) might return 7 items with a `LastEvaluatedKey` indicating more data exists. The current code interprets `items.len() <= page_size` as "no next page," which is wrong.

3. **The cursor is opaque to the application.** `LastEvaluatedKey` is a DynamoDB-internal structure. It cannot be derived from the result set; only DynamoDB can produce it.

But this is not just a DynamoDB problem. SQL has a related issue:

4. **ORDER BY columns may not be in the SELECT list.** Consider `SELECT name FROM users ORDER BY age, id`. The cursor needs `[age, id]` but neither is in the projection. Today, `extract_cursor` reads values from the returned row, so if the ORDER BY columns aren't there, cursor construction fails. The engine needs to load those extra columns from the database and strip them before returning results to the user.

Both problems point to the same conclusion: **pagination must be an engine concern**, not application-layer logic in `Paginate::exec()`.

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

For the SQL ORDER BY case, pagination also happens entirely in `Paginate::exec()`. The engine has no opportunity to add missing ORDER BY columns to the SELECT list, because it doesn't know a paginated query is happening — it just sees a regular query with a limit.

### Existing Engine Pattern: Load Extra Columns, Project Away

The engine already has the machinery for this. The back-ref projection flow (`plan/statement.rs:1287-1310`) does exactly what pagination needs:

1. `load_data.columns` accumulates columns needed by child statements beyond what the user requested
2. The database returns all of them
3. `Project` nodes extract what's needed for internal use (child statement inputs)
4. The final projection strips the extras before returning to the user

Pagination can use the same pattern: during planning, add ORDER BY columns to `load_data.columns`, build an `extract_cursor` function targeting those column positions, and insert a `Project` to strip them from the output.

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
- **Does not address the SQL ORDER BY problem.** If ORDER BY columns aren't in the SELECT list, `Paginate::exec()` still can't extract cursor values from the rows. This approach only fixes DynamoDB; the SQL cursor extraction issue remains.

### B: Engine-aware pagination

The engine owns pagination end-to-end. Pagination flows through lower → plan → exec as a first-class concept.

For SQL, the planner adds ORDER BY columns to `load_data.columns` if they aren't already present, builds an `extract_cursor` eval function targeting those column positions, and inserts a `Project` to strip them from the output. This follows the same pattern as back-ref projections.

For DynamoDB, the `QueryPk` operation carries a `pagination: Option<PaginationRequest>` that tells the driver to return a cursor token. The driver sets `page_size` as `Limit`, passes back `LastEvaluatedKey` as the cursor, and the engine propagates it to `Page`.

```rust
// Pagination config attached to MIR actions
pub struct PaginationConfig {
    pub page_size: i64,
    /// SQL: extract cursor values from loaded columns (including non-projected ones)
    pub extract_cursor: Option<eval::Func>,
}

// Driver operation
pub struct QueryPk {
    // ... existing fields ...
    pub pagination: Option<PaginationRequest>,
}

pub struct PaginationRequest {
    pub page_size: i64,
    pub resume_token: Option<CursorToken>,
}

// Driver response
pub struct Response {
    pub rows: Rows,
    pub cursor_token: Option<CursorToken>,
}
```

The engine's executor handles both paths uniformly:

- **SQL path:** Executor collects rows, applies `extract_cursor` to the last row (which includes the extra ORDER BY columns), strips those columns via `Project`, and produces the cursor as `CursorToken` (serialized keyset values).
- **DynamoDB path:** Executor reads `cursor_token` from `Response`. No N+1 needed — `has_next` is `cursor_token.is_some()`.

**Pros:**

- Solves both DynamoDB and SQL problems. DynamoDB gets `LastEvaluatedKey` support; SQL gets ORDER BY columns added to the load and projected away.
- Uses existing engine machinery. The `load_data.columns` → `Project` pattern is already proven for back-ref projections.
- Uniform output shape. `Page` gets cursors from the engine regardless of backend. `Paginate::exec()` becomes a thin wrapper that calls the engine and reads back metadata.
- The N+1 strategy lives in the right place. For SQL, the engine adds +1 to the limit and checks overflow. For DynamoDB, the driver signals via `cursor_token` presence. Both happen inside the engine, not in application code.

**Cons:**

- Largest change surface. Pagination awareness must flow through the statement AST (or be derived during lowering), the planner, MIR, and executor. The existing `pagination.md` design doc describes 5 phases of work.
- The planner must distinguish between SQL and DynamoDB paths to decide whether to use `extract_cursor` or `cursor_token`. This is gated by `driver.capability()`, which already exists, but it's still a branch.
- The `CursorToken` type at the driver boundary is opaque bytes. DynamoDB's `LastEvaluatedKey` serialization format becomes a driver-internal concern, which is clean but harder to debug than typed `stmt::Value` cursors.

### C: Opaque cursor tokens at the driver boundary

Pagination stays in `Paginate::exec()` but cursors become opaque `Vec<u8>` blobs that only the originating driver can interpret. The engine is a pass-through: it receives a `CursorToken` from `Paginate`, forwards it to the driver, and passes the driver's response token back.

```rust
pub struct CursorToken(pub Vec<u8>);

// Driver operation
pub struct QueryPk {
    // ... existing fields ...
    pub cursor_token: Option<CursorToken>,  // replaces cursor: Option<stmt::Value>
}

// Driver response
pub struct Response {
    pub rows: Rows,
    pub cursor_token: Option<CursorToken>,
}
```

For DynamoDB, `CursorToken` is a serialized `LastEvaluatedKey`. For SQL drivers, the driver itself adds ORDER BY columns to the query, extracts cursor values from the result, and serializes them into the token.

**Pros:**

- Clean abstraction boundary. The engine never inspects or constructs cursors. Drivers own the full lifecycle: produce token → store opaquely → receive token → resume query.
- Handles DynamoDB GSI pagination correctly by definition — the token is the actual `LastEvaluatedKey`.
- No `stmt::Value` serialization gymnastics. DynamoDB's `HashMap<String, AttributeValue>` maps naturally to bytes without fitting it into Toasty's value type system.
- Works for future backends (e.g., Cassandra paging state tokens) without changes to the engine.

**Cons:**

- **Pushes the SQL ORDER BY problem into the driver.** The SQL driver must now modify the query to add ORDER BY columns, execute it, extract cursor values, and strip the extra columns — all logic that the engine is better positioned to do (it already has `load_data.columns` and `Project` for this). This duplicates engine-level patterns inside each SQL driver.
- Breaks the current design where `Paginate` manipulates cursor values as expressions. `Page::next()` and `Page::prev()` currently embed cursors into the statement AST via `Offset::After(Expr)`. With opaque tokens, the flow changes entirely.
- Opaque tokens are harder to debug. A `stmt::Value` cursor is inspectable; a `Vec<u8>` requires driver-specific deserialization.
- Each SQL driver (SQLite, PostgreSQL, MySQL) would independently implement keyset pagination logic. This is redundant — the `toasty-sql` crate exists precisely to share SQL generation, but cursor extraction happens after execution, outside SQL generation.

## Comparison

| Concern | A: Response metadata | B: Engine-aware | C: Opaque tokens |
|---|---|---|---|
| DynamoDB GSI correctness | Yes | Yes | Yes |
| DynamoDB N+1 problem | Needs `has_more` flag | Engine checks cursor presence | Driver signals via token |
| SQL ORDER BY not in SELECT | **Not addressed** | Handled (load_data + Project) | Driver handles (duplicated) |
| Change surface | Small | Large | Medium |
| Abstraction leakage | Driver concept in shared interface | Capability-gated branches | Clean boundary |
| Reuses existing patterns | No | Yes (back-ref projection) | No |
| Incremental delivery | Easy (DynamoDB only) | Harder (both paths needed) | Medium |
| Debugging | Good (typed values) | Good (typed values in engine) | Harder (opaque bytes) |
| Future backends | Needs per-backend fields | Capability-gated | Works naturally |

## Recommendation

Approach B. Both DynamoDB and SQL pagination have the same structural need: the engine must manage cursor lifecycle because only the engine sits between the user's projection and the database's response. The engine already has the load-extra-columns-then-project-away pattern from back-ref handling. Pagination fits naturally into this.

The implementation path:

1. Introduce a `Paginate` variant in the statement AST (or derive pagination intent during lowering from `Limit` + `OrderBy`).
2. During planning, if pagination is active:
   - **SQL:** Add ORDER BY columns to `load_data.columns`. Build `extract_cursor` as an `eval::Func` targeting those positions. Apply N+1 to the limit. Insert a `Project` to strip extra columns.
   - **NoSQL:** Set `pagination: Some(PaginationRequest { page_size, resume_token })` on the `QueryPk` operation.
3. During execution:
   - **SQL:** Collect rows, check for N+1 overflow, apply `extract_cursor` to the last kept row, serialize as `CursorToken`, project away extra columns.
   - **NoSQL:** Read `cursor_token` from `Response`. `has_next = cursor_token.is_some()`.
4. The executor produces an `ExecResponse { values: ValueStream, cursor_token: Option<CursorToken> }` that propagates through `VarStore` to the final result.
5. `Paginate::exec()` becomes a thin wrapper: call engine, read `cursor_token` from result, build `Page`.

The `CursorToken` at the boundary between engine and `Page` is opaque — the engine serializes SQL keyset values into it, and the DynamoDB driver serializes `LastEvaluatedKey` into it. When `Page::next()` is called, the token flows back into the engine, which either deserializes it into WHERE filters (SQL) or passes it through to the driver (DynamoDB).
