//! Legalize engine-final statements for the target backend.
//!
//! The engine's internal phases (simplify, lower, plan) produce one canonical
//! statement form regardless of backend, and some constructs in that form
//! have no driver representation. Legalization is the last engine-side
//! transformation before a statement crosses the driver boundary: driven by
//! [`Capability`], it rewrites each such construct into an equivalent form
//! the target backend can represent.
//!
//! Legalization preserves semantics. A rule changes how an operation is
//! expressed, never what it computes — a backend that cannot express an
//! operation at all is rejected earlier, by `verify`. Syntax differences
//! between backends that share a representation (quoting, placeholders) are
//! the serializer's job (`toasty-sql`), not legalization's.
//!
//! One rule module exists today: [`document`], which rewrites `#[document]`
//! path reads into the resolved [`FuncJsonExtract`](stmt::FuncJsonExtract)
//! name paths drivers consume. Future per-backend rewrites join it as sibling
//! modules, invoked from [`statement`].
//!
//! Entry points: [`Engine::prepare_for_driver`] legalizes a full statement
//! and extracts its bind parameters; [`table_expr`] legalizes a bare table
//! expression that crosses the boundary inside a key-value operation.

use super::Engine;
use toasty_core::{
    driver::{Capability, operation::TypedValue},
    schema::{Schema, db},
    stmt,
};

mod document;

impl Engine {
    /// Prepare an engine-final statement to cross the driver boundary:
    /// legalize it for the target backend, then (on SQL backends) extract its
    /// bind parameters. The returned `Vec<TypedValue>` is indexed by the `n`
    /// in each `Expr::Arg(n)` placeholder; key-value backends read values
    /// directly from the statement, so the vec is empty.
    ///
    /// Every driver-bound statement passes through this method, after the
    /// last engine-side mutation (e.g. the MySQL `RETURNING` rewrites) and
    /// immediately before the driver serializes it.
    pub(crate) fn prepare_for_driver(&self, stmt: &mut stmt::Statement) -> Vec<TypedValue> {
        statement(&self.schema, self.capability(), stmt);

        if self.capability().sql {
            super::bind::run(stmt, &self.schema.db, self.capability())
        } else {
            vec![]
        }
    }
}

/// Legalize a full statement: run every rule module over it.
fn statement(schema: &Schema, capability: &Capability, stmt: &mut stmt::Statement) {
    document::statement(schema, capability, stmt);
}

/// Legalize a driver-bound expression that references `table`'s columns — a
/// key-value operation's filter or condition, which the driver compiles
/// without an enclosing statement.
pub(crate) fn table_expr(
    schema: &Schema,
    capability: &Capability,
    table: &db::Table,
    expr: &mut stmt::Expr,
) {
    document::table_expr(schema, capability, table, expr);
}
