//! Tracing spans for statement execution.
//!
//! Statements execute on a dedicated per-connection worker task, so a span
//! entered by the caller does not naturally cover the driver's work. The
//! spans created here are constructed on the caller's task — parenting them
//! to the caller's current span (e.g. a per-request span) — then carried
//! across the channel to the worker task and entered there, so every event
//! the engine and driver emit lands inside the caller's span tree.

use toasty_core::{
    Schema,
    schema::app::ModelId,
    stmt::{self, ExprSet, InsertTarget, UpdateTarget},
};

/// Creates the `toasty::query` span for one statement execution.
///
/// When the span is disabled by the subscriber's filter, returns the
/// caller's current span instead so request context still reaches the
/// worker task's events.
pub(crate) fn query_span(schema: &Schema, stmt: &stmt::Statement) -> tracing::Span {
    let span = tracing::debug_span!(
        target: "toasty::query",
        "query",
        stmt.kind = stmt.name(),
        model = tracing::field::Empty,
    );
    if span.is_disabled() {
        return tracing::Span::current();
    }
    if let Some(model_id) = stmt_model_id(stmt) {
        let name = schema.app.model(model_id).name().upper_camel_case();
        span.record("model", name.as_str());
    }
    span
}

/// Creates the `toasty::query` span for a user-authored SQL execution.
/// Falls back to the caller's current span like [`query_span`].
pub(crate) fn raw_sql_span() -> tracing::Span {
    let span = tracing::debug_span!(target: "toasty::query", "query", stmt.kind = "raw_sql");
    if span.is_disabled() {
        return tracing::Span::current();
    }
    span
}

/// Resolves the model a statement targets, when it targets one directly.
/// Statements arriving here are model-level; lowered (table-level) shapes
/// return `None`.
fn stmt_model_id(stmt: &stmt::Statement) -> Option<ModelId> {
    match stmt {
        stmt::Statement::Query(query) => query_model_id(query),
        stmt::Statement::Insert(insert) => match &insert.target {
            InsertTarget::Model(model_id) => Some(*model_id),
            InsertTarget::Scope(query) => query_model_id(query),
            InsertTarget::Table(_) => None,
        },
        stmt::Statement::Update(update) => match &update.target {
            UpdateTarget::Model(model_id) => Some(*model_id),
            UpdateTarget::Query(query) => query_model_id(query),
            UpdateTarget::Table(_) => None,
        },
        stmt::Statement::Delete(delete) => delete.from.as_model().map(|source| source.id),
    }
}

fn query_model_id(query: &stmt::Query) -> Option<ModelId> {
    match &query.body {
        ExprSet::Select(select) => select.source.as_model().map(|source| source.id),
        ExprSet::Update(update) => match &update.target {
            UpdateTarget::Model(model_id) => Some(*model_id),
            UpdateTarget::Query(query) => query_model_id(query),
            UpdateTarget::Table(_) => None,
        },
        _ => None,
    }
}
