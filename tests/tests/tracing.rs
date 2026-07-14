#![cfg(feature = "sqlite")]

//! Tests for the `toasty::query` tracing event and span propagation across
//! the per-connection worker task.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tracing::{Instrument, Level};
use tracing_subscriber::{Layer, layer::SubscriberExt, registry::LookupSpan};

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i64,
    #[index]
    name: String,
}

async fn setup_db(configure: impl FnOnce(&mut toasty::db::Builder)) -> toasty::Db {
    let mut builder = toasty::Db::builder();
    builder.models(toasty::models!(User));
    configure(&mut builder);
    let db = builder
        .build(toasty_driver_sqlite::Sqlite::in_memory())
        .await
        .unwrap();
    db.push_schema().await.unwrap();
    db
}

/// One captured tracing event: its metadata, flattened fields, and the
/// names and fields of every span in scope (root first).
#[derive(Debug, Clone)]
struct CapturedEvent {
    target: String,
    level: Level,
    fields: HashMap<String, String>,
    scope: Vec<(String, HashMap<String, String>)>,
}

impl CapturedEvent {
    fn message(&self) -> &str {
        self.fields.get("message").map(String::as_str).unwrap_or("")
    }
}

#[derive(Default)]
struct FieldVisitor(HashMap<String, String>);

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{value:?}"));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
}

/// Per-span field storage, kept in the registry's extensions.
struct SpanFields(HashMap<String, String>);

#[derive(Clone)]
struct CaptureLayer {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
    max_level: Level,
}

impl<S> Layer<S> for CaptureLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    // Force a dynamic `enabled` check on every event. Tests run in
    // parallel, each with its own thread-local subscriber; a cached
    // per-callsite Interest computed against one test's subscriber must
    // not decide event delivery for another's. (This is also why the
    // level cutoff lives here instead of in `Layer::with_filter` — the
    // `Filtered` wrapper keys off state set in `enabled()`, which the
    // global Interest cache can skip.)
    fn register_callsite(
        &self,
        _metadata: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::sometimes()
    }

    fn enabled(
        &self,
        metadata: &tracing::Metadata<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        *metadata.level() <= self.max_level
    }

    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = FieldVisitor::default();
        attrs.record(&mut visitor);
        let span = ctx.span(id).unwrap();
        span.extensions_mut().insert(SpanFields(visitor.0));
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = FieldVisitor::default();
        values.record(&mut visitor);
        let span = ctx.span(id).unwrap();
        if let Some(fields) = span.extensions_mut().get_mut::<SpanFields>() {
            fields.0.extend(visitor.0);
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if *event.metadata().level() > self.max_level {
            return;
        }
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        let mut scope = vec![];
        if let Some(event_scope) = ctx.event_scope(event) {
            for span in event_scope.from_root() {
                let fields = span
                    .extensions()
                    .get::<SpanFields>()
                    .map(|f| f.0.clone())
                    .unwrap_or_default();
                scope.push((span.name().to_string(), fields));
            }
        }

        self.events.lock().unwrap().push(CapturedEvent {
            target: event.metadata().target().to_string(),
            level: *event.metadata().level(),
            fields: visitor.0,
            scope,
        });
    }
}

/// Installs a capturing subscriber for the current thread and returns the
/// shared event log plus the guard keeping the subscriber active.
fn capture() -> (
    Arc<Mutex<Vec<CapturedEvent>>>,
    tracing::subscriber::DefaultGuard,
) {
    capture_at(Level::TRACE)
}

/// Like [`capture`], but only events at `max_level` or above reach the
/// capturing layer — mimics a production `RUST_LOG` filter.
fn capture_at(
    max_level: Level,
) -> (
    Arc<Mutex<Vec<CapturedEvent>>>,
    tracing::subscriber::DefaultGuard,
) {
    let layer = CaptureLayer {
        events: Arc::default(),
        max_level,
    };
    let events = layer.events.clone();
    let subscriber = tracing_subscriber::registry().with(layer);
    let guard = tracing::subscriber::set_default(subscriber);
    (events, guard)
}

fn query_events(events: &Arc<Mutex<Vec<CapturedEvent>>>) -> Vec<CapturedEvent> {
    events
        .lock()
        .unwrap()
        .iter()
        .filter(|ev| ev.target == "toasty::query")
        .cloned()
        .collect()
}

#[tokio::test]
async fn query_event_has_statement_duration_and_rows() {
    let (events, _guard) = capture();
    let mut db = setup_db(|_| {}).await;

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await
        .unwrap();
    events.lock().unwrap().clear();

    let users = User::filter_by_name("Alice").exec(&mut db).await.unwrap();
    assert_eq!(users.len(), 1);

    let captured = query_events(&events);
    assert_eq!(captured.len(), 1, "captured: {captured:#?}");

    let ev = &captured[0];
    assert_eq!(ev.level, Level::DEBUG);
    assert_eq!(ev.message(), "query executed");
    assert_eq!(ev.fields["db.system"], "sqlite");
    assert!(ev.fields["db.statement"].contains("SELECT"));
    assert!(ev.fields.contains_key("duration_ms"));
    assert_eq!(ev.fields["rows"], "1");
    // Off by default: bound parameter values must not appear.
    assert!(!ev.fields.contains_key("db.params"));
}

#[tokio::test]
async fn params_logged_only_when_opted_in() {
    let (events, _guard) = capture();
    let mut db = setup_db(|builder| {
        builder.log_statement_params(true);
    })
    .await;

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await
        .unwrap();
    events.lock().unwrap().clear();

    User::filter_by_name("Alice").exec(&mut db).await.unwrap();

    let captured = query_events(&events);
    let ev = &captured[0];
    assert_eq!(ev.fields["db.params"], "[\"Alice\"]");
}

#[tokio::test]
async fn slow_statement_escalates_to_warn() {
    let (events, _guard) = capture();
    let mut db = setup_db(|builder| {
        builder.slow_statement_threshold(Some(Duration::ZERO));
    })
    .await;

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await
        .unwrap();
    events.lock().unwrap().clear();

    User::filter_by_name("Alice").exec(&mut db).await.unwrap();

    let captured = query_events(&events);
    let ev = &captured[0];
    assert_eq!(ev.level, Level::WARN);
    assert_eq!(ev.message(), "slow query");
}

/// A `RUST_LOG=warn`-style filter disables the DEBUG event but still emits
/// slow queries at WARN; opted-in params must appear on those events too.
#[tokio::test]
async fn params_present_on_slow_query_under_warn_filter() {
    let (events, _guard) = capture_at(Level::WARN);
    let mut db = setup_db(|builder| {
        builder
            .log_statement_params(true)
            .slow_statement_threshold(Some(Duration::ZERO));
    })
    .await;

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await
        .unwrap();
    events.lock().unwrap().clear();

    User::filter_by_name("Alice").exec(&mut db).await.unwrap();

    let captured = query_events(&events);
    let all: Vec<_> = events.lock().unwrap().clone();
    let ev = captured
        .first()
        .unwrap_or_else(|| panic!("no toasty::query events; all captured: {all:#?}"));
    assert_eq!(ev.level, Level::WARN);
    assert_eq!(ev.fields["db.params"], "[\"Alice\"]");
}

/// Regression test for tokio-rs/toasty#1043: driver events must land inside
/// the caller's span even though the driver runs on a dedicated worker task.
#[tokio::test]
async fn query_event_inherits_caller_span() {
    let (events, _guard) = capture();
    let mut db = setup_db(|_| {}).await;

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await
        .unwrap();
    events.lock().unwrap().clear();

    let request_span = tracing::info_span!("request");
    async {
        User::filter_by_name("Alice").exec(&mut db).await.unwrap();
    }
    .instrument(request_span)
    .await;

    let captured = query_events(&events);
    let ev = &captured[0];
    let scope_names: Vec<&str> = ev.scope.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(scope_names, ["request", "query"], "scope: {:#?}", ev.scope);

    // The toasty query span carries the statement kind and resolved model.
    let (_, query_fields) = &ev.scope[1];
    assert_eq!(query_fields["stmt.kind"], "query");
    assert_eq!(query_fields["model"], "User");
}
