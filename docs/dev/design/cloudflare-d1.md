# Cloudflare D1 Driver

## Summary

Toasty adds a Cloudflare D1 driver for Rust applications running in a
Cloudflare Worker. Applications construct the driver from a request-local D1
binding. Toasty uses D1's SQLite-compatible SQL interface, executes eligible
multi-statement plans through D1's atomic batch API, and rejects operations
that require an interactive transaction.

## Motivation

D1 exposes a relational database to Workers through a runtime binding rather
than a connection URL. The binding does not create independent connections and
cannot remain valid beyond the Worker request that supplied it. D1 also differs
from connected SQLite in two ways that affect Toasty's execution model:

- Workers cannot hold an interactive SQL transaction across calls.
- D1 can execute a fixed list of prepared statements as one atomic batch.

Treating D1 as a pooled SQLite connection would start pool machinery that the
binding does not need and would send transaction operations that D1 does not
support. A D1 driver needs request-local connection ownership and an explicit
atomic-batch contract.

## User-facing API

Add Toasty, the D1 driver, and the Workers SDK to the Worker crate:

```toml
[dependencies]
toasty = "0.9"
toasty-driver-d1 = "0.9"
worker = { version = "0.8", features = ["d1"] }
```

Read the binding from the Worker environment and build one `Db` for the
request:

```rust
use toasty_driver_d1::D1;
use worker::{Context, Env, Request, Response, Result, event};

#[event(fetch)]
pub async fn fetch(
    _request: Request,
    env: Env,
    _context: Context,
) -> Result<Response> {
    let driver = D1::new("DB", env.d1("DB")?);
    let mut db = toasty::Db::builder()
        .models(toasty::models!(crate::*))
        .build(driver)
        .await
        .map_err(|error| worker::Error::RustError(error.to_string()))?;

    // Use `db` before the request returns.
    Response::ok("ready")
}
```

The driver lives in `toasty-driver-d1` rather than behind a feature on
`toasty`. A feature could re-export the driver, but it could not construct one:
the application must still read the request-local binding from `Env` and pass
it to `D1::new`.

Pool settings do not apply to D1. `Db::builder()` rejects settings such as
`max_pool_size` and `pool_pre_ping` when building a direct driver.

### Batches and transactions

Call `toasty::batch()` when every statement can be prepared before the batch
starts:

```rust
let (user, post) = toasty::batch((
    User::create().name("Alice"),
    Post::create().title("Hello"),
))
.exec(&mut db)
.await?;
```

Toasty sends eligible multi-statement plans through D1's atomic batch API. D1
executes the statements in order and rolls back the group if a statement
fails.

`db.transaction()` is not available. A plan that must read one database result
before constructing a later statement also returns
`Error::UnsupportedFeature`. The application must express the work as one SQL
statement, split it into separate non-atomic operations, or use a database that
supports interactive transactions.

### Schema management

`db.push_schema()` creates tables and indexes through one D1 atomic batch. It
is suitable for local development and disposable databases.

Production schema changes use Wrangler migrations. The D1 migration generator
emits SQLite-compatible SQL, including deferred foreign-key checking for table
rebuilds, but Toasty does not apply or track those migrations at runtime.

## Behavior

The driver serializes generated statements with Toasty's SQLite serializer and
uses numbered question-mark parameters. Generated queries, raw SQL, inserts,
updates, deletes, result projections, and supported relation queries retain
their SQLite SQL behavior.

D1 values cross a JavaScript API boundary. The driver rejects integers outside
JavaScript's safe integer range and non-finite floating-point values rather
than changing their values during conversion. UUID values use canonical text
because D1's binding API cannot carry Toasty's SQLite UUID blob representation.
Document values, scalar lists, decimal values, and date/time values use their
existing text encodings.

The driver validates D1 limits that can be determined before dispatch,
including parameter count, SQL length, pattern length, column count, and
individual string or blob size. D1 remains responsible for aggregate limits
such as final row size.

A single database operation executes directly. For a plan with multiple
database operations, the engine selects one of these forms from the driver's
capabilities:

- An interactive transaction streams operations between `BEGIN` and `COMMIT`.
- An atomic SQL batch sends an ordered list of generated statements at once.
- A driver supporting neither form rejects the plan before dispatch.

For D1, every database operation in an atomic plan must be a generated SQL
statement with no dependency on an earlier database result. The driver returns
one result for each input statement in the same order. A missing, extra, or
malformed result returns `Error::InvalidResult`.

Errors from the Workers SDK or D1 result objects become Toasty driver-operation
errors. Unsupported operations, including interactive transactions, runtime
migrations, and database reset, return `Error::UnsupportedFeature`.

## Edge cases

**Request lifetime.** A `D1` value can create one direct Toasty connection.
Cloned `Db` handles share that connection, and access is serialized. The
application must not retain the `Db` after its Worker request ends.

**Empty schemas.** `push_schema()` returns successfully without sending an
empty D1 batch.

**Atomic dependencies.** Pagination, conditional output, raw SQL operations,
and statements whose inputs depend on earlier database results are not
eligible for an atomic SQL batch. Toasty rejects the complete plan before
sending its first statement.

**Integer representation.** Signed and unsigned integer fields share
JavaScript's safe integer ceiling even though SQLite can store the full signed
64-bit range. Model validation and value binding report the same range.

**Query timing.** `std::time::Instant` is unavailable on
`wasm32-unknown-unknown`. D1 query events retain their SQL, parameters, result,
and row metadata but report zero duration.

**Migration table rebuilds.** D1 enforces foreign keys inside its implicit
transactions. Rebuild migrations use `PRAGMA defer_foreign_keys = ON` instead
of disabling foreign keys. Connected SQLite keeps its existing behavior.

## Driver integration

`Driver::connection_strategy()` tells `Db` how to own connections. Its default
is `ConnectionStrategy::Pooled`, so existing drivers and out-of-tree drivers do
not need to implement it. A direct driver returns
`ConnectionStrategy::Direct`; `Db` then calls `Driver::connect()` once and
serializes all access to that connection without starting pool background
tasks.

`Capability` describes transaction delivery separately from whether the
backend uses SQL:

- `interactive_transactions` allows `Transaction` operations.
- `atomic_batch` allows a fixed group of generated SQL statements.
- `max_bind_parameters` reports the backend ceiling; the D1 driver rejects
  oversized statements before dispatch.
- `runtime_migrations` and `reset_database` describe administrative support.
- `defer_foreign_keys_during_rebuild` selects D1-compatible rebuild SQL.

The additional `Capability` fields require out-of-tree drivers that construct
the struct directly to select values when updating Toasty. Existing shipped
drivers retain their previous transaction and administration behavior.

`Connection::exec_atomic_batch()` accepts an ordered `AtomicSqlBatch` and
returns one `ExecResponse` per statement. Its default implementation returns
`Error::UnsupportedFeature`; a sequential fallback is invalid because it would
weaken the requested atomicity. Implementors must commit the complete list or
return an error without committing a prefix.

SQL drivers that support interactive transactions continue to receive
individual `Operation` values through `Connection::exec()`. They do not need
to implement the atomic-batch method.

## Alternatives considered

**Use the existing connection pool with a maximum size of one.** This still
models the binding as a factory, permits reconnect attempts after its one value
has been consumed, and retains pool timers and background tasks. A direct
connection represents the binding's ownership and lifetime explicitly.

**Expose D1 through `Db::connect()`.** D1 has no connection URL, and only the
Worker environment can supply the binding. A URL form would hide a value that
the application must provide.

**Emulate interactive transactions by buffering operations.** The engine may
need an earlier result to construct a later operation, so buffering cannot
preserve interactive transaction behavior. Toasty rejects those plans instead
of changing their semantics.

**Execute multi-statement plans sequentially.** This can leave partial state
after an error and breaks Toasty's atomic batch guarantee. Toasty uses D1's
batch API or rejects the plan.

**Put D1 behind a `toasty` feature.** A feature would only re-export the driver;
it would not remove the need to depend on the Workers SDK and read the binding.
Keeping the driver separate isolates its Wasm-only dependencies and runtime
API.

## Open questions

No open questions remain for this implementation. Adding D1 Sessions for read
replication can be designed independently without changing the binding-based
construction API.

## Out of scope

- **Interactive transactions and savepoints.** The Worker binding does not
  expose them.
- **Runtime migration tracking.** Wrangler owns D1 migration files and the
  `d1_migrations` ledger.
- **Database reset.** D1 does not expose Toasty's connected-database reset
  workflow through the binding driver.
- **D1 Sessions and read replication.** The initial driver executes against the
  binding without a session API.
- **Automatic retries.** Retrying a write after an ambiguous failure could
  apply it twice.
- **Using D1 outside a Worker.** The driver targets the Workers binding API, not
  the Cloudflare REST API.
