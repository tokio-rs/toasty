# Batch Query Execution

## Overview

Batch queries let users send multiple independent queries to the database in a
single round-trip. The results come back as a typed tuple matching the input
queries.

```rust
let (active_users, recent_posts) = toasty::batch((
    User::find_by_active(true),
    Post::find_recent(100),
)).exec(&db).await?;

// active_users: Vec<User>
// recent_posts: Vec<Post>
```

Without batching, each query requires its own trip through the engine and driver.
Batching amortizes the connection overhead and, for SQL databases, sends all
queries in a single protocol exchange.

This design covers SQL databases only. DynamoDB support is out of scope.

## User-Facing API

### `toasty::batch()`

```rust
pub fn batch<Q: BatchQuery>(queries: Q) -> Batch<Q> {
    Batch { queries }
}

pub struct Batch<Q> {
    queries: Q,
}

impl<Q: BatchQuery> Batch<Q> {
    pub async fn exec(
        self,
        executor: &mut dyn Executor,
    ) -> Result<Q::Output> {
        // ...
    }
}
```

`batch()` accepts any tuple of query builders (anything that implements
`IntoSelect`). It returns a `Batch` handle whose `exec` method runs all queries
and returns the results as a typed tuple.

### `BatchQuery` trait

```rust
pub trait BatchQuery {
    /// The result type. For a 2-tuple of queries, this is
    /// `(Vec<A>, Vec<B>)`.
    type Output;

    /// Collect the untyped statements from each query.
    fn into_statements(self) -> Vec<stmt::Statement>;

    /// Reconstruct the typed output from a vec of value streams,
    /// one per statement in the same order as `into_statements`.
    fn from_streams(streams: Vec<ValueStream>) -> impl Future<Output = Result<Self::Output>>;
}
```

Tuple implementations connect the typed world to the untyped engine:

```rust
impl<A, B> BatchQuery for (A, B)
where
    A: IntoSelect,
    A::Model: Load,
    B: IntoSelect,
    B::Model: Load,
{
    type Output = (Vec<A::Model>, Vec<B::Model>);

    fn into_statements(self) -> Vec<stmt::Statement> {
        vec![
            self.0.into_select().untyped.into(),
            self.1.into_select().untyped.into(),
        ]
    }

    async fn from_streams(streams: Vec<ValueStream>) -> Result<Self::Output> {
        let mut iter = streams.into_iter();
        let a: Vec<A::Model> = Cursor::new(iter.next().unwrap()).collect().await?;
        let b: Vec<B::Model> = Cursor::new(iter.next().unwrap()).collect().await?;
        Ok((a, b))
    }
}
```

Implementations for tuples of arity 2 through 8 (or some reasonable upper bound)
are generated with a macro.

## Execution Path

### 1. Application layer

`Batch::exec` calls `BatchQuery::into_statements()` to collect the untyped
statements, then passes them to a new `Executor` method.

### 2. Executor

Add a new method to `Executor`:

```rust
#[async_trait]
pub trait Executor: Send + Sync {
    // ... existing methods ...

    /// Execute multiple statements and return one value stream per statement.
    async fn exec_batch(
        &mut self,
        stmts: Vec<toasty_core::stmt::Statement>,
    ) -> Result<Vec<ValueStream>>;
}
```

The default implementation runs each statement sequentially through
`exec_untyped`. SQL drivers can override this to concatenate the queries into
a single protocol round-trip.

### 3. Engine

Each statement in the batch is compiled independently through the existing
pipeline (lower → plan → exec). The statements are unrelated, so no
cross-statement optimization is needed.

A new `Engine::exec_batch` method handles this:

```rust
pub(crate) async fn exec_batch(
    &self,
    connection: &mut PoolConnection,
    stmts: Vec<Statement>,
    in_transaction: bool,
) -> Result<Vec<ValueStream>> {
    let mut results = Vec::with_capacity(stmts.len());
    for stmt in stmts {
        results.push(self.exec(connection, stmt, in_transaction).await?);
    }
    Ok(results)
}
```

This initial implementation compiles and runs statements one at a time. It still
reduces round-trips at the driver level because all compiled operations share the
same connection and can be sent together.

### 4. Db / ConnectionOperation

Add a new `ConnectionOperation` variant:

```rust
pub(crate) enum ConnectionOperation {
    // ... existing variants ...
    ExecBatch {
        stmts: Vec<Box<toasty_core::stmt::Statement>>,
        in_transaction: bool,
        tx: oneshot::Sender<Result<Vec<ValueStream>>>,
    },
}
```

The background connection task handles this by calling `engine.exec_batch`.

### 5. SQL driver batching

For SQL databases, the driver can concatenate multiple queries separated by
semicolons and execute them in a single `Connection::exec` call. This requires a
new `Operation` variant:

```rust
pub enum Operation {
    // ... existing variants ...
    BatchQuerySql(BatchQuerySql),
}

pub struct BatchQuerySql {
    pub queries: Vec<QuerySql>,
}
```

The planner detects when it is producing a batch and emits `BatchQuerySql`
instead of individual `QuerySql` operations. The SQL driver serializes all
queries into one SQL string (semicolon-separated), executes them, and splits the
result sets back into separate `ValueStream`s.

Whether the driver supports this depends on the database client library:

| Driver | Multi-statement support |
|--------|----------------------|
| SQLite (rusqlite) | `execute_batch` or multiple `prepare`/`query` calls |
| PostgreSQL (tokio-postgres) | `simple_query` supports multi-statement |
| MySQL (mysql_async) | Needs `CLIENT_MULTI_STATEMENTS` flag |

If a driver does not support multi-statement execution, it falls back to
sequential execution (one query at a time on the same connection).

## Implementation Plan

### Phase 1: Foundation

Introduce the public API types and sequential execution. No driver-level
batching.

1. Add `BatchQuery` trait with tuple impls (2- through 8-tuples via macro)
2. Add `toasty::batch()` function and `Batch` struct
3. Add `Executor::exec_batch` with a default sequential implementation
4. Add `ConnectionOperation::ExecBatch` variant and handle it in `Db`
5. Wire `Batch::exec` → `Executor::exec_batch` → `Engine::exec` (called in a
   loop)

After this phase, the API works and all queries execute on the same connection
sequentially. The round-trip savings come from reusing one connection instead
of acquiring N connections.

### Phase 2: SQL multi-statement

Send all queries to the database in a single protocol exchange.

1. Add `Operation::BatchQuerySql` to the driver interface
2. Update the SQLite driver to handle `BatchQuerySql`
3. Update the PostgreSQL driver
4. Update the MySQL driver
5. Update the engine planner to emit `BatchQuerySql` when executing a batch

### Phase 3: Polish

1. Add `first` / `get` variants (return `Option<M>` / `M` instead of `Vec<M>`)
2. Support mixing select, insert, update, and delete in a batch
3. Error handling: decide whether a single failure aborts the whole batch or
   returns partial results

## Design Decisions

### Why a trait instead of a method per arity?

A `BatchQuery` trait with tuple impls keeps the public API to a single function
(`toasty::batch(queries).exec(&db)`) regardless of how many queries are batched.
Adding new arities is a macro expansion, not new API surface.

### Why `IntoSelect` instead of a new `IntoStatement` trait?

Query builders already implement `IntoSelect`. Using it avoids introducing a new
trait and changing codegen. If batch support expands to inserts and updates later,
an `IntoStatement` trait can be added at that point and `BatchQuery` impls can
accept it.

### Why sequential engine compilation?

Statements in a batch are independent — they query different models with
different filters. There is no cross-statement optimization opportunity.
Compiling them sequentially through the existing pipeline is simple and correct.
The performance win comes from the driver sending them together, not from
compile-time merging.

### Why not wrap in an implicit transaction?

Batch queries are read-only selects in the common case. Wrapping them in a
transaction adds overhead and changes isolation semantics. Users who need
transactional consistency can combine batching with `db.transaction()`.

## Files Modified

| File | Change |
|------|--------|
| `crates/toasty/src/batch.rs` | Add `batch()`, `Batch`, `BatchQuery` trait, tuple impls |
| `crates/toasty/src/lib.rs` | Re-export `batch`, `Batch`, `BatchQuery` |
| `crates/toasty/src/executor.rs` | Add `exec_batch` method |
| `crates/toasty/src/db.rs` | Add `ExecBatch` variant, implement `exec_batch` |
| `crates/toasty/src/engine.rs` | Add `exec_batch` method |
| `crates/toasty/src/transaction.rs` | Implement `exec_batch` (delegates to sequential) |
| `crates/toasty-core/src/driver/operation.rs` | Add `BatchQuerySql` (phase 2) |
| `crates/toasty-driver-sqlite/src/lib.rs` | Handle `BatchQuerySql` (phase 2) |
| `crates/toasty-driver-postgresql/src/lib.rs` | Handle `BatchQuerySql` (phase 2) |
| `crates/toasty-driver-mysql/src/lib.rs` | Handle `BatchQuerySql` (phase 2) |

## Integration Tests

Test cases for the driver integration suite:

- Batch two selects on different models, verify both return correct results
- Batch a select that returns rows with a select that returns empty
- Batch with filters, ordering, and limits
- Batch inside a transaction
- Batch of a single query (degenerates to normal execution)
