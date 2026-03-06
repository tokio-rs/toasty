# Plan: Implement Database `now()` Function

## Problem Statement

Toasty currently handles timestamps via app-level Rust expressions (`jiff::Timestamp::now()`) evaluated at builder construction time. The roadmap calls for supporting the database-level `NOW()` function, which provides:
- **Clock consistency**: all records use the database server's clock, not the application's
- **Statement-level atomicity**: within a single SQL statement, `NOW()` returns the same value for all rows

The core challenge: **how does the user get the value back after a mutation?**
- PostgreSQL and SQLite support `RETURNING` — straightforward
- MySQL does **not** support `RETURNING`, and unlike auto-increment IDs (where `LAST_INSERT_ID()` works), there is no MySQL function to retrieve the exact `NOW()` value used in a prior statement

## Design Decision: Hybrid Evaluation Strategy

When a `now()` expression appears in a mutation (INSERT/UPDATE):

| Driver capability | Strategy | User gets back... |
|---|---|---|
| `returning_from_mutation: true` (PostgreSQL, SQLite) | Send `NOW()` / `CURRENT_TIMESTAMP` to the database; read value back via `RETURNING` | The exact database timestamp |
| `returning_from_mutation: false` (MySQL) | **Resolve `now()` app-side** before sending the query — substitute a concrete `jiff::Timestamp` value, send that literal to the database | The app-evaluated timestamp (sent to DB as a literal) |
| DynamoDB (no SQL) | **Resolve `now()` app-side** — DynamoDB has no SQL functions | The app-evaluated timestamp |

This is the same pattern already used for auto-increment IDs: PostgreSQL/SQLite use `RETURNING`, while MySQL uses the `last_insert_id_hack`. The difference is that for `now()`, MySQL's fallback is even simpler — we just evaluate the timestamp in Rust and send a literal value. The user always gets a timestamp back; on PostgreSQL/SQLite it's the database's clock, on MySQL/DynamoDB it's the app's clock.

### Why not use MySQL's `NOW()` + a follow-up `SELECT NOW()`?

MySQL's `NOW()` returns the **statement** start time, not the transaction start time. A subsequent `SELECT NOW()` in the same transaction would return a different value (the start time of *that* statement). There's no reliable way to retrieve the exact `NOW()` from a prior statement without `RETURNING`.

## Implementation Steps

### Step 1: Add `now()` to the Statement AST

**File:** `crates/toasty-core/src/stmt/expr_func.rs`

Add a `Now` variant to the `ExprFunc` enum:

```rust
pub enum ExprFunc {
    Count(FuncCount),
    LastInsertId(FuncLastInsertId),
    Now(FuncNow),  // NEW
}
```

**New file:** `crates/toasty-core/src/stmt/func_now.rs`

```rust
/// The `NOW()` / `CURRENT_TIMESTAMP` function.
///
/// Returns the current date and time from the database server.
/// On databases that don't support RETURNING (MySQL), this is resolved
/// app-side to a concrete timestamp before the query is sent.
#[derive(Clone, Debug, PartialEq)]
pub struct FuncNow;
```

**File:** `crates/toasty-core/src/stmt.rs`

Export `FuncNow` from the stmt module.

### Step 2: Add SQL serialization for `NOW()`

**File:** `crates/toasty-sql/src/serializer/expr.rs`

Add a case for the `Now` variant in the expression serializer:

```rust
Func(stmt::ExprFunc::Now(_)) => {
    // All SQL dialects support CURRENT_TIMESTAMP (it's SQL standard)
    // PostgreSQL/MySQL also support NOW() but CURRENT_TIMESTAMP is universal
    fmt!(cx, f, "CURRENT_TIMESTAMP")
},
```

Use `CURRENT_TIMESTAMP` rather than `NOW()` because it's SQL-standard and works across all three SQL dialects (PostgreSQL, MySQL, SQLite).

### Step 3: Add a user-facing `toasty::now()` function

**New file or addition to:** `crates/toasty/src/stmt/` (e.g., `now.rs` or add to an existing module)

Provide a public function that returns an `Expr<jiff::Timestamp>` wrapping `ExprFunc::Now`:

```rust
/// Returns a database-level `NOW()` expression.
///
/// When used in a mutation (insert/update), the database evaluates this
/// at statement execution time. On databases without RETURNING support
/// (MySQL), the value is evaluated app-side instead.
pub fn now() -> Expr<jiff::Timestamp> {
    Expr::from_stmt(stmt::Expr::Func(stmt::ExprFunc::Now(stmt::FuncNow)))
}
```

This lets users write:
```rust
#[default(toasty::now())]
created_at: jiff::Timestamp,
```

Or use it in manual query building.

### Step 4: Resolve `now()` before execution on non-RETURNING drivers

**File:** `crates/toasty/src/engine/exec/exec_statement.rs`

Add a resolution pass that runs before sending the statement to the driver. This pass walks the statement AST and, **if the driver does not support `returning_from_mutation`**, replaces all `ExprFunc::Now` nodes with a concrete `Expr::Value` containing the current `jiff::Timestamp`.

```rust
fn resolve_now_if_needed(&self, stmt: &mut stmt::Statement) {
    if self.engine.capability().returning_from_mutation {
        // Database supports RETURNING; let the DB evaluate NOW()
        return;
    }

    // Replace all Now() expressions with concrete timestamps
    let now = jiff::Timestamp::now();
    stmt::visit_mut::for_each_expr_mut(stmt, |expr| {
        if matches!(expr, stmt::Expr::Func(stmt::ExprFunc::Now(_))) {
            *expr = stmt::Expr::Value(/* convert jiff::Timestamp to stmt::Value */);
        }
    });
}
```

This runs **before** the existing `process_stmt_insert_with_returning_on_mysql` logic, so by the time MySQL processing happens, there are no `Now()` expressions left — they've been replaced with literal timestamp values. This means the RETURNING clause (if present) only contains concrete values, and the existing MySQL workaround for reconstructing RETURNING results works without modification.

**For DynamoDB:** The DynamoDB driver uses key-value operations, not SQL. The `now()` resolution should happen at the same stage — the planner/executor resolves `Now()` to a concrete value before generating DynamoDB operations.

### Step 5: Handle RETURNING with `now()` on PostgreSQL/SQLite

On databases that support RETURNING, the `NOW()` expression flows through to SQL naturally:

```sql
INSERT INTO posts (title, created_at) VALUES ('Hello', CURRENT_TIMESTAMP)
RETURNING id, created_at;
```

The database evaluates `CURRENT_TIMESTAMP`, inserts it, and returns the actual value in the RETURNING result set. The existing RETURNING processing code in the engine already handles reading back arbitrary column values, so this should work with minimal changes.

**Verify:** The RETURNING column type mapping handles timestamp types correctly. Check that `stmt::Type::Timestamp` is mapped properly in the result set deserialization for each driver.

### Step 6: Wire up `#[auto]` shorthand (optional enhancement)

Currently `#[auto]` on `created_at` expands to `#[default(jiff::Timestamp::now())]` (Rust-side). Consider updating this to `#[default(toasty::now())]` so that `#[auto]` on timestamp fields uses the database clock by default on supporting databases. This is a behavior change and could be done as a follow-up.

### Step 7: Integration tests

**File:** `crates/toasty-driver-integration-suite/src/tests/` (new test file, e.g., `db_now.rs`)

Tests to add:
1. **Insert with `now()` default** — verify the returned model has a non-zero timestamp
2. **Update with `now()` expression** — verify the timestamp changes on update
3. **Multiple inserts in one statement** — verify all rows get the same timestamp (on databases where `NOW()` is statement-level)
4. **Explicit override** — verify user can still set an explicit timestamp that overrides `now()`
5. **Round-trip** — insert with `now()`, then query back, verify the timestamps match

These tests run against all drivers via `generate_driver_tests!`, so they automatically validate behavior on SQLite, PostgreSQL, MySQL, and DynamoDB.

## Files to Modify/Create

| File | Change |
|---|---|
| `crates/toasty-core/src/stmt/func_now.rs` | **NEW** — `FuncNow` struct |
| `crates/toasty-core/src/stmt/expr_func.rs` | Add `Now(FuncNow)` variant |
| `crates/toasty-core/src/stmt.rs` | Export `FuncNow` |
| `crates/toasty-sql/src/serializer/expr.rs` | Serialize `Now` → `CURRENT_TIMESTAMP` |
| `crates/toasty/src/stmt/` | Add public `toasty::now()` function |
| `crates/toasty/src/engine/exec/exec_statement.rs` | Resolve `now()` to literal on non-RETURNING drivers |
| `crates/toasty-driver-integration-suite/src/tests/db_now.rs` | **NEW** — integration tests |

## Open Questions

1. **Should `#[auto]` on `created_at`/`updated_at` switch to `toasty::now()`?** This would change the default behavior from app-clock to database-clock (on PostgreSQL/SQLite). Could be a follow-up.

2. **Should `now()` work in queries (not just mutations)?** E.g., `Post::filter(|p| p.created_at.lt(toasty::now()))`. This is useful but orthogonal — `NOW()` in a SELECT WHERE clause doesn't have the RETURNING problem. Could be supported from day one since it's simpler.

3. **Precision:** `CURRENT_TIMESTAMP` precision varies by database (PostgreSQL defaults to microseconds, MySQL defaults to seconds unless you specify `CURRENT_TIMESTAMP(6)`). Should we emit `CURRENT_TIMESTAMP(6)` on MySQL? This should match the column precision configured in the capability system (`default_timestamp_type: db::Type::DateTime(6)`).
