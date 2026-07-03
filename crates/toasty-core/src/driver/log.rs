//! Per-query tracing instrumentation shared by all drivers.
//!
//! Every driver emits one event per physical database operation through
//! [`QueryLog`], so all backends produce the same shape: target
//! `toasty::query` at `DEBUG` (escalated to `WARN` past the configured
//! slow-statement threshold), with the statement text or operation name,
//! elapsed time, row count, and outcome. Field names follow the
//! [OpenTelemetry database semantic conventions] (the two-segment forms —
//! `db.system`, `db.statement` — since `tracing` macros reject field names
//! with more than two dot-separated segments).
//!
//! [OpenTelemetry database semantic conventions]: https://opentelemetry.io/docs/specs/semconv/database/

use crate::driver::{ExecResponse, Rows};
use crate::stmt::Value;

use std::fmt::Write;
use std::time::{Duration, Instant};

/// The tracing target of the per-query event, shared by all drivers.
pub const TARGET: &str = "toasty::query";

/// Maximum number of parameter values rendered per statement.
const MAX_PARAMS: usize = 32;

/// Maximum number of characters rendered per string parameter.
const MAX_STR_LEN: usize = 128;

/// Maximum number of items rendered per list parameter.
const MAX_LIST_ITEMS: usize = 8;

/// Configuration for the per-query `toasty::query` tracing event.
///
/// Set on `Db::builder()` and handed to each driver connection by the
/// connection pool via
/// [`Connection::set_query_log_config`](super::Connection::set_query_log_config).
#[derive(Debug, Clone, Copy)]
pub struct QueryLogConfig {
    /// Include bound parameter values in the event. Off by default:
    /// parameter values are application data and may contain secrets.
    pub params: bool,

    /// Emit the per-query event at `WARN` instead of `DEBUG` when
    /// execution takes at least this long. `None` disables escalation.
    pub slow_statement_threshold: Option<Duration>,
}

impl Default for QueryLogConfig {
    fn default() -> Self {
        Self {
            params: false,
            slow_statement_threshold: Some(Duration::from_secs(1)),
        }
    }
}

/// Measures one in-flight database operation and emits the `toasty::query`
/// event when finished.
///
/// Construct with [`sql`](Self::sql) or [`operation`](Self::operation)
/// immediately before executing, then call [`finish`](Self::finish) with the
/// execution result. Timing runs from construction to `finish`.
#[derive(Debug)]
pub struct QueryLog<'a> {
    config: QueryLogConfig,
    system: &'static str,
    query_text: Option<&'a str>,
    operation: Option<&'a str>,
    collection: Option<&'a str>,
    params: Option<String>,
    rows: Option<u64>,
    start: Instant,
}

impl<'a> QueryLog<'a> {
    /// Starts measuring a SQL statement execution.
    pub fn sql<'v>(
        config: &QueryLogConfig,
        system: &'static str,
        sql: &'a str,
        params: impl IntoIterator<Item = &'v Value>,
    ) -> Self {
        Self {
            config: *config,
            system,
            query_text: Some(sql),
            operation: None,
            collection: None,
            params: render_params(config, params),
            rows: None,
            start: Instant::now(),
        }
    }

    /// Starts measuring a key-value operation execution.
    pub fn operation(
        config: &QueryLogConfig,
        system: &'static str,
        operation: &'a str,
        collection: Option<&'a str>,
    ) -> Self {
        Self {
            config: *config,
            system,
            query_text: None,
            operation: Some(operation),
            collection,
            params: None,
            rows: None,
            start: Instant::now(),
        }
    }

    /// Records the number of rows returned, for drivers that know it before
    /// handing back a stream. Row counts for count-style responses are read
    /// from the [`ExecResponse`] in [`finish`](Self::finish) automatically.
    pub fn rows(&mut self, rows: u64) {
        self.rows = Some(rows);
    }

    /// Emits the `toasty::query` event describing `result`.
    pub fn finish(self, result: &crate::Result<ExecResponse>) {
        let elapsed = self.start.elapsed();
        let duration_ms = elapsed.as_secs_f64() * 1e3;
        let rows = self.rows.or(match result {
            Ok(response) => match &response.values {
                Rows::Count(count) => Some(*count),
                _ => None,
            },
            Err(_) => None,
        });
        let slow = self
            .config
            .slow_statement_threshold
            .is_some_and(|threshold| elapsed >= threshold);

        let error = result.as_ref().err().map(tracing::field::display);
        let message = match (slow, result) {
            (_, Err(_)) => "query failed",
            (true, Ok(_)) => "slow query",
            (false, Ok(_)) => "query executed",
        };

        // Strings are recorded through `Display` so the default `fmt`
        // subscriber prints them bare — SQL full of quoted identifiers is
        // unreadable once escaped.
        let system = tracing::field::display(self.system);
        let statement = self.query_text.map(tracing::field::display);
        let operation = self.operation.map(tracing::field::display);
        let collection = self.collection.map(tracing::field::display);
        let params = self.params.as_deref().map(tracing::field::display);

        // `duration_ms` leads because the tracing macros mis-parse a dotted
        // field name placed directly after `target:`.
        if slow {
            tracing::warn!(
                target: "toasty::query",
                duration_ms,
                rows,
                error,
                db.system = system,
                db.statement = statement,
                db.operation = operation,
                db.collection = collection,
                db.params = params,
                "{message}"
            );
        } else {
            tracing::debug!(
                target: "toasty::query",
                duration_ms,
                rows,
                error,
                db.system = system,
                db.statement = statement,
                db.operation = operation,
                db.collection = collection,
                db.params = params,
                "{message}"
            );
        }
    }
}

/// Renders parameter values to a bounded, human-readable list, or `None`
/// when param logging is disabled or the event could not be observed anyway.
/// Rendering happens before execution, when it is not yet known whether the
/// event fires at `DEBUG` or (past the slow threshold) `WARN`, so both
/// callsites are checked: a `RUST_LOG=warn` filter must still get params on
/// slow-query events.
fn render_params<'v>(
    config: &QueryLogConfig,
    params: impl IntoIterator<Item = &'v Value>,
) -> Option<String> {
    let enabled = tracing::event_enabled!(target: "toasty::query", tracing::Level::DEBUG)
        || tracing::event_enabled!(target: "toasty::query", tracing::Level::WARN);
    if !config.params || !enabled {
        return None;
    }

    let mut out = String::from("[");
    for (i, value) in params.into_iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        if i == MAX_PARAMS {
            out.push_str("...");
            break;
        }
        render_value(&mut out, value);
    }
    out.push(']');
    Some(out)
}

fn render_value(out: &mut String, value: &Value) {
    let _ = match value {
        Value::Null => {
            out.push_str("NULL");
            Ok(())
        }
        Value::Bool(v) => write!(out, "{v}"),
        Value::I8(v) => write!(out, "{v}"),
        Value::I16(v) => write!(out, "{v}"),
        Value::I32(v) => write!(out, "{v}"),
        Value::I64(v) => write!(out, "{v}"),
        Value::U8(v) => write!(out, "{v}"),
        Value::U16(v) => write!(out, "{v}"),
        Value::U32(v) => write!(out, "{v}"),
        Value::U64(v) => write!(out, "{v}"),
        Value::F32(v) => write!(out, "{v}"),
        Value::F64(v) => write!(out, "{v}"),
        Value::Uuid(v) => write!(out, "{v}"),
        Value::String(s) => {
            render_str(out, s);
            Ok(())
        }
        Value::Bytes(b) => write!(out, "<{} bytes>", b.len()),
        Value::List(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                if i == MAX_LIST_ITEMS {
                    out.push_str("...");
                    break;
                }
                render_value(out, item);
            }
            out.push(']');
            Ok(())
        }
        other => write!(out, "{other:?}"),
    };
}

fn render_str(out: &mut String, s: &str) {
    let total = s.chars().count();
    if total <= MAX_STR_LEN {
        let _ = write!(out, "{s:?}");
    } else {
        let prefix: String = s.chars().take(MAX_STR_LEN).collect();
        let _ = write!(out, "{prefix:?}...({total} chars)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(value: Value) -> String {
        let mut out = String::new();
        render_value(&mut out, &value);
        out
    }

    #[test]
    fn renders_scalars_bare() {
        assert_eq!(rendered(Value::Null), "NULL");
        assert_eq!(rendered(Value::Bool(true)), "true");
        assert_eq!(rendered(Value::I64(-42)), "-42");
        assert_eq!(rendered(Value::F64(1.5)), "1.5");
    }

    #[test]
    fn renders_strings_quoted_and_truncated() {
        assert_eq!(rendered(Value::String("hi".into())), "\"hi\"");

        let long = "x".repeat(500);
        let out = rendered(Value::String(long));
        assert!(out.starts_with('"'));
        assert!(out.ends_with("...(500 chars)"));
        assert!(out.len() < 200);
    }

    #[test]
    fn renders_bytes_as_length() {
        assert_eq!(rendered(Value::Bytes(vec![0; 16])), "<16 bytes>");
    }

    #[test]
    fn caps_list_items() {
        let list = Value::List((0..20).map(Value::I64).collect());
        assert_eq!(rendered(list), "[0, 1, 2, 3, 4, 5, 6, 7, ...]");
    }
}
