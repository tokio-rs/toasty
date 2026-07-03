# Tracing

Toasty emits structured spans and events through the [`tracing`] crate.
Toasty does not print anything by default — install a subscriber in your
application and the events appear.

[`tracing`]: https://docs.rs/tracing

## The query event

Every driver emits one event per database operation, after the operation
completes, on the target `toasty::query`. The event carries the statement,
elapsed time, row count, and outcome, so a single filter shows every
database round-trip regardless of backend:

```text
DEBUG request:query: toasty::query: query executed duration_ms=0.4 rows=1
    db.system=sqlite db.statement=SELECT tbl_0_0."id", tbl_0_0."name" FROM
    "users" AS tbl_0_0 WHERE tbl_0_0."name" = ?1
```

Install [`tracing-subscriber`] with the `env-filter` feature and run with
`RUST_LOG=toasty::query=debug`:

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
        .connect("sqlite::memory:")
        .await?;

    // ... queries here ...
    Ok(())
}
```

The event's fields:

| Field | Meaning |
|---|---|
| `db.system` | Backend that ran the operation: `sqlite`, `turso`, `postgresql`, `mysql`, or `dynamodb`. |
| `db.statement` | The serialized SQL, with `?N` (SQLite, Turso, MySQL) or `$N` (PostgreSQL) placeholders. SQL drivers only. |
| `db.operation` | The operation name (`get_by_key`, `query_pk`, `scan`, …). Key-value drivers only. |
| `db.collection` | The table the operation targets. Key-value drivers only. |
| `db.params` | Bound parameter values. Only present when enabled — see [Parameter values](#parameter-values). |
| `duration_ms` | Elapsed execution time in milliseconds. |
| `rows` | Rows returned or affected, when the driver knows the count. |
| `error` | The error, when the operation failed. |

The field names follow [OpenTelemetry's database semantic conventions], so
subscribers that understand those conventions (for example, an OTLP
exporter) pick the fields up without extra mapping. `db.statement` is
recorded as a string, so the SQL appears bare in the default `fmt`
subscriber — no surrounding quotes, no escaping of the identifier quotes
inside.

[OpenTelemetry's database semantic conventions]: https://opentelemetry.io/docs/specs/semconv/database/

## The query span

Each statement executes inside a `toasty::query` span named `query`, with
two fields:

| Field | Meaning |
|---|---|
| `stmt.kind` | `query`, `insert`, `update`, `delete`, or `raw_sql`. |
| `model` | The model the statement targets (e.g. `User`), when it targets one. |

The span is created on the task that calls Toasty, so it is parented to
whatever span is current there — a per-request span created by axum or
tower middleware, for example. Toasty executes statements on a dedicated
worker task per pooled connection; the span is carried across to that task
and entered there, so query events (and everything else Toasty logs during
execution) land inside the calling request's span tree:

```text
DEBUG request{rid=324580}:query{stmt.kind="query" model="User"}: toasty::query: query executed ...
```

## Slow queries

When a statement takes at least one second, its query event is emitted at
`WARN` instead of `DEBUG`, so slow queries surface in production logs
without enabling debug output. Configure the threshold on the builder:

```rust,ignore
let db = toasty::Db::builder()
    .models(toasty::models!(User))
    // Escalate statements slower than 250ms.
    .slow_statement_threshold(Some(std::time::Duration::from_millis(250)))
    .connect("sqlite::memory:")
    .await?;
```

Pass `None` to disable the escalation.

## Parameter values

By default the query event does not include bound parameter values —
they are application data and may contain secrets. Opt in on the builder:

```rust,ignore
let db = toasty::Db::builder()
    .models(toasty::models!(User))
    .log_statement_params(true)
    .connect("sqlite::memory:")
    .await?;
```

The values appear in the `db.params` field, in placeholder order, in a
bounded form: long strings are truncated, byte blobs are summarized by
length, and long lists are capped.

```text
DEBUG toasty::query: query executed ... db.statement=SELECT ... WHERE tbl_0_0."name" = ?1 db.params=["Alice"]
```

Enable this only when the log destination is trusted.

## Custom formatting

The query event carries everything as structured fields, so a custom
[`Layer`] can reformat it however you like — for example, rendering
parameter values inline into the SQL for copy-paste into a database
client. Match on the `toasty::query` target and read the fields:

[`Layer`]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/layer/trait.Layer.html

```rust,ignore
use tracing_subscriber::Layer;

struct SqlLogLayer;

impl<S: tracing::Subscriber> Layer<S> for SqlLogLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if event.metadata().target() != "toasty::query" {
            return;
        }

        #[derive(Default)]
        struct Fields {
            statement: Option<String>,
            params: Option<String>,
            duration_ms: Option<f64>,
        }
        impl tracing::field::Visit for Fields {
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                match field.name() {
                    "db.statement" => self.statement = Some(value.to_string()),
                    "db.params" => self.params = Some(value.to_string()),
                    _ => {}
                }
            }
            fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
                if field.name() == "duration_ms" {
                    self.duration_ms = Some(value);
                }
            }
            fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
        }

        let mut fields = Fields::default();
        event.record(&mut fields);
        if let Some(sql) = fields.statement {
            println!(
                "SQL ({:.1}ms): {} -- params: {}",
                fields.duration_ms.unwrap_or(0.0),
                sql,
                fields.params.as_deref().unwrap_or("[]"),
            );
        }
    }
}
```

Register it alongside your other layers:

```rust,ignore
use tracing_subscriber::layer::SubscriberExt;

let subscriber = tracing_subscriber::registry().with(SqlLogLayer);
tracing::subscriber::set_global_default(subscriber)?;
```

⚠️ If you render parameter values into the SQL string, treat the result as
debug output only — never execute it. Placeholder substitution is not
escaping-aware.

## Level policy

Toasty assigns levels by audience, so a production filter like
`RUST_LOG=warn,toasty=info` stays quiet until something needs attention:

| Level | What it reports |
|---|---|
| `ERROR` | Toasty itself failed in a way it cannot recover from (e.g. the connection pool cannot be built). |
| `WARN` | Statements past the slow-statement threshold, and configuration mismatches (e.g. the driver caps the pool below the requested size). |
| `INFO` | One-time lifecycle events: schema build, database ready, applied migrations. |
| `DEBUG` | One event per query (the `toasty::query` event), transaction begin/commit/rollback, and pool connection lifecycle: creation, discards, health-check failures. |
| `TRACE` | Engine internals: execution plan actions, per-operation driver dispatch, decoded result dumps. |

Two principles keep the higher levels quiet:

- Errors that propagate to the caller are reported at `DEBUG`, not
  `ERROR` — the caller decides whether they are application errors. This
  covers failed statements (the query event's `error` field) and pool
  acquire failures. A unique-constraint violation your code handles is
  not an error worth logging twice.
- The pool recovering from a dead connection — an idle timeout, a
  server restart, a failed ping — is normal operation, not an anomaly,
  so discards and health-check failures log at `DEBUG`.

## Filtering

`RUST_LOG` accepts per-target directives:

```sh
# Just the per-query events
RUST_LOG=toasty::query=debug cargo run

# Everything Toasty logs at debug, but only PostgreSQL internals at trace
RUST_LOG=toasty=debug,toasty_driver_postgresql=trace cargo run

# Lifecycle only
RUST_LOG=toasty=info cargo run
```

All drivers emit the query event on the same `toasty::query` target; use
the `db.system` field to tell backends apart in an application that uses
more than one.
