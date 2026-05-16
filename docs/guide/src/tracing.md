# Tracing

Toasty emits structured events through the [`tracing`] crate. The most
common reason to enable them is to see the SQL each query produces.
Toasty does not print anything by default — install a subscriber in your
application and the events appear.

[`tracing`]: https://docs.rs/tracing

## Seeing executed SQL

The SQL drivers emit a `tracing::debug!` event for every statement they
send to the database. Install [`tracing-subscriber`] with the
`env-filter` feature and run with `RUST_LOG=toasty=debug`:

[`tracing-subscriber`]: https://docs.rs/tracing-subscriber

```toml
[dependencies]
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

```rust,ignore
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> toasty::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let db = toasty::Db::builder()
        .models(toasty::models!(crate::*))
        .connect("postgresql://user:pass@localhost/mydb")
        .await?;

    // ... queries here ...
    Ok(())
}
```

Run the program with:

```sh
RUST_LOG=toasty=debug cargo run
```

Each query produces two events. The engine logs the statement kind, and
the driver logs the SQL it sends:

```text
DEBUG toasty::engine: executing statement stmt.kind="query"
DEBUG toasty_driver_sqlite: executing SQL db.system="sqlite" db.statement=SELECT tbl_0_0."id", tbl_0_0."name" FROM "users" AS tbl_0_0 WHERE tbl_0_0."id" = ?1; params=1
```

The SQL event carries three fields:

| Field | Meaning |
|---|---|
| `db.system` | Driver that ran the statement: `sqlite`, `postgresql`, or `mysql`. |
| `db.statement` | The serialized SQL, with `?N` (SQLite, MySQL) or `$N` (PostgreSQL) placeholders for parameters. |
| `params` | Number of bound parameters. The values themselves are not logged. |

`db.statement` is recorded with the `Display` representation, so the SQL
appears bare in the default `fmt` subscriber — no surrounding quotes,
no escaping of the identifier quotes inside.

Parameter values are passed to the driver as typed bindings and are not
included in the trace. To inspect a specific parameter, log it from your
application code.

The field names follow [OpenTelemetry's database semantic conventions],
so subscribers that understand those conventions (for example, an OTLP
exporter) pick the SQL up without extra mapping.

[OpenTelemetry's database semantic conventions]: https://opentelemetry.io/docs/specs/semconv/database/

## Filtering to one driver

`RUST_LOG` accepts per-target directives. To see SQL from one driver
only, filter on its crate:

```sh
# Only PostgreSQL statements
RUST_LOG=toasty_driver_postgresql=debug cargo run

# Only SQLite statements
RUST_LOG=toasty_driver_sqlite=debug cargo run

# Only MySQL statements
RUST_LOG=toasty_driver_mysql=debug cargo run
```

## Other events

The `info` level reports lifecycle events: schema build, database ready,
and applied migrations. Enable it with `RUST_LOG=toasty=info`.

At `debug`, the engine also dumps the decoded result of each statement
(`Final result from var ...`) alongside the SQL event. This is verbose
on large result sets; filter it out with
`RUST_LOG=toasty=debug,toasty::engine::exec=info` if you only want the
SQL.

The `trace` level adds per-operation detail — driver dispatch, execution
plan size, and transaction begin/commit/rollback. It is verbose; reach
for it when `debug` does not show enough.

```sh
RUST_LOG=toasty=trace cargo run
```

## DynamoDB

The DynamoDB driver does not run SQL, but it emits `tracing::trace!`
events for each item operation it performs (`getting single item`,
`querying primary key`, `batch inserting items`, and so on) with the
table name, index name, and item counts. Enable them with
`RUST_LOG=toasty_driver_dynamodb=trace`.
