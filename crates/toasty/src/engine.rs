mod eval;
mod exec;

mod plan;
use plan::Plan;

mod planner;
mod simplify;
mod ty;
mod verify;

use crate::{Db, Result};
use toasty_core::stmt::{Expr, Query, Statement, ValueStream};

/// Response from executing a statement, containing the value stream and optional metadata
#[derive(Debug)]
pub struct ExecResponse {
    /// The stream of values returned by the statement
    pub values: ValueStream,
    /// Optional metadata about the execution (e.g., pagination cursors)
    pub metadata: Option<Metadata>,
}

/// Metadata returned from statement execution
#[derive(Debug, Clone)]
pub struct Metadata {
    /// Cursor pointing to the next page (for pagination)
    pub next_cursor: Option<Expr>,
    /// Cursor pointing to the previous page (for pagination)
    pub prev_cursor: Option<Expr>,
    /// The original query that was executed
    pub query: Query,
}

pub(crate) async fn exec(db: &Db, stmt: Statement) -> Result<ExecResponse> {
    if cfg!(debug_assertions) {
        verify::apply(&db.schema, &stmt);
    }

    // Translate the optimized statement into a series of driver operations.
    let plan = planner::apply(db.driver.capability(), &db.schema, stmt)?;

    // The plan is called once (single entry record stream) with no arguments
    // (empty record).
    // exec::exec now returns ExecResponse directly
    exec::exec(db, &plan.pipeline, plan.vars).await
}
