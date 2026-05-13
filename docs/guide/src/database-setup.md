# Database Setup

Opening a database connection in Toasty has two steps: register your models,
then connect to a database. `Db::builder()` handles both.

```rust,ignore
let mut db = toasty::Db::builder()
    .models(toasty::models!(User, Post))
    .connect("sqlite::memory:")
    .await?;
```

## Registering models

The `models!` macro builds a `ModelSet` — the collection of model definitions
Toasty uses to generate the database schema. It accepts three forms, which can
be combined freely:

```rust,ignore
toasty::models!(
    // All models from the current crate
    crate::*,
    // All models from an external crate
    third_party_models::*,
    // Individual models
    User,
    other_module::Post,
)
```

`crate::*` finds all `#[derive(Model)]` and `#[derive(Embed)]` types in your
crate at compile time. This is the simplest option when all your models live in
one crate.

You don't need to list every model. Registering a model also registers any
models reachable through its fields — `BelongsTo`, `HasMany`, `HasOne`, and
embedded types are all discovered by traversing the model's fields. For
example, if `User` has a `HasMany<Post>` field and `Post` has a `BelongsTo<User>`
field, `toasty::models!(User)` registers both `User` and `Post`.

## Connection URLs

The connection URL determines which database driver Toasty uses. Each driver
requires its corresponding feature flag in `Cargo.toml`.

| Scheme | Database | Feature flag |
|---|---|---|
| `sqlite` | SQLite | `sqlite` |
| `postgresql` or `postgres` | PostgreSQL | `postgresql` |
| `mysql` | MySQL | `mysql` |
| `dynamodb` | DynamoDB | `dynamodb` |

Examples:

```rust,ignore
// In-memory SQLite
.connect("sqlite::memory:")

// SQLite file
.connect("sqlite://path/to/db.sqlite")

// PostgreSQL
.connect("postgresql://user:pass@localhost:5432/mydb")

// MySQL
.connect("mysql://user:pass@localhost:3306/mydb")

// DynamoDB (uses AWS config from environment)
.connect("dynamodb://us-east-1")
```

### PostgreSQL connection options

PostgreSQL accepts query parameters in the URL:

| Parameter | Purpose |
|---|---|
| `application_name=<string>` | Reported to PostgreSQL as the connecting application name. Shows up in `pg_stat_activity` and the server log. |
| `sslmode=<mode>` | TLS mode. Requires the `tls` feature on `toasty-driver-postgresql`. |
| `sslrootcert=<path>` | Path to a PEM-encoded root certificate to trust. |
| `sslcert=<path>` and `sslkey=<path>` | Paths to a client certificate and matching private key, for mutual TLS. |
| `channel_binding=<mode>` | `disable`, `prefer` (default), or `require`. |
| `sslnegotiation=<mode>` | `postgres` (default) or `direct`. |

Supported `sslmode` values: `disable`, `prefer` (default), `require`,
`verify-ca`, `verify-full`. Without the `tls` feature, any `sslmode`
other than `disable` fails at connect time.

```rust,ignore
.connect("postgresql://user:pass@localhost/mydb?sslmode=require&application_name=myservice")
```

## Using a driver directly

If you need more control over the driver configuration, construct the driver
yourself and pass it to `build()` instead of `connect()`:

```rust,ignore
let driver = toasty_driver_sqlite::Sqlite::in_memory();
let mut db = toasty::Db::builder()
    .models(toasty::models!(User))
    .build(driver)
    .await?;
```

## Connection pool

`Db` owns a connection pool. Each query checks out a connection from the
pool for the duration of the call and returns it when finished. The pool
defaults work for typical applications; the builder exposes knobs for
tuning size, timeouts, and broken-connection recovery.

```rust,ignore
use std::time::Duration;

let mut db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .max_pool_size(32)
    .pool_wait_timeout(Some(Duration::from_secs(5)))
    .pool_create_timeout(Some(Duration::from_secs(10)))
    .connect("postgresql://user:pass@localhost/mydb")
    .await?;
```

| Builder method | Default | Purpose |
|---|---|---|
| `max_pool_size(n)` | `num_cpus * 2` | Cap on simultaneous open connections. Drivers may enforce a lower cap (e.g., in-memory SQLite is single-connection). |
| `pool_wait_timeout(Some(d))` | `None` | Maximum time `Db` waits for a free connection before returning an error. `None` waits indefinitely. |
| `pool_create_timeout(Some(d))` | `None` | Maximum time to spend opening a new connection. |
| `pool_health_check_interval(Some(d))` | `Some(60s)` | How often the background sweep pings an idle connection to detect a silently-broken backend. `None` disables the sweep. |
| `pool_pre_ping(true)` | `false` | Ping every connection before handing it to the caller. Adds one round-trip per checkout in exchange for guaranteeing the caller sees a live connection. |

### Recovering from a backend restart

A database restart, a load-balancer-closed socket, or a backend session
timeout leaves the pool holding TCP sockets that look open but reject
the next query. Toasty handles this two ways:

- **Background sweep.** Every `pool_health_check_interval`, the pool
  pings one idle connection. If the ping fails, the pool drops the
  failing connection and eagerly pings the rest of the idle slots so a
  single bad result drains every dead connection in one pass.
- **Reactive sweep.** When a user query observes a connection-lost
  error, the same eager sweep runs immediately. A backend restart
  typically costs one failed user query rather than one per pooled
  connection.

Enable `pool_pre_ping(true)` if even one failed query is unacceptable —
for example, a public API behind a flaky network or an idempotent
worker without retry. The cost is one extra round-trip per checkout.

## Table name prefix

To prefix all generated table names (useful when multiple services share a
database), call `table_name_prefix()` on the builder:

```rust,ignore
let mut db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .table_name_prefix("myapp_")
    .connect("sqlite::memory:")
    .await?;
```
