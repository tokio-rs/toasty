pub(crate) mod eval;
pub(crate) mod exec;

mod extract_params;
#[cfg(test)]
pub(crate) mod test_util;

mod fold;
mod hir;
use hir::HirStatement;

mod index;
mod lower;
mod mir;
mod plan;
mod select_item;
pub(crate) use select_item::{SelectItem, SelectItems};
mod simplify;
mod ty;
mod verify;

use crate::Result;
use std::sync::Arc;
use toasty_core::{
    Connection, Schema,
    driver::Capability,
    stmt::{self, Statement},
};

/// The query execution engine.
///
/// [`Engine`] orchestrates the multi-phase compilation pipeline that transforms
/// user queries into database operations. It owns the schema and capability
/// reference, and provides the main entry point ([`exec`](Self::exec)) for
/// executing statements.
///
/// The execution pipeline follows this process:
///
/// 1. **Verification.** Validate statement structure and reject AST shapes
///    the driver does not support.
/// 2. **Lowering.** Convert to HIR with dependency tracking.
/// 3. **Planning.** Build MIR operation graph.
/// 4. **Execution.** Run actions against the database driver.
#[derive(Debug, Clone)]
pub(crate) struct Engine {
    /// The schema being managed by this database instance.
    pub(crate) schema: Arc<Schema>,

    /// Driver capabilities, used during planning.
    pub(crate) capability: &'static Capability,
}

impl Engine {
    /// Creates a new [`Engine`] with the given schema and capability.
    pub(crate) fn new(schema: Arc<Schema>, capability: &'static Capability) -> Engine {
        Engine { schema, capability }
    }

    /// Returns the driver's capabilities.
    pub(crate) fn capability(&self) -> &Capability {
        self.capability
    }

    /// Executes a statement and returns the full response including pagination metadata.
    ///
    /// The statement passes through the full compilation pipeline
    /// (lowering -> planning -> execution) before being sent to the database
    /// driver via the provided connection.
    pub(crate) async fn exec(
        &self,
        connection: &mut dyn Connection,
        stmt: Statement,
        in_transaction: bool,
    ) -> Result<toasty_core::driver::ExecResponse> {
        tracing::debug!(stmt.kind = stmt.name(), "executing statement");

        self.verify(&stmt)?;

        if let stmt::Statement::Insert(stmt) = &stmt {
            assert!(matches!(
                stmt.returning,
                Some(stmt::Returning::Model { .. })
            ));
        }

        // Lower the statement to High-level intermediate representation
        let hir = self.lower_stmt(stmt)?;

        // Translate the optimized statement into a series of driver operations.
        let plan = self.plan_hir_statement(hir)?;

        tracing::trace!(
            actions = plan.actions.len(),
            needs_transaction = plan.needs_transaction,
            "execution plan ready"
        );

        // The plan is called once (single entry record stream) with no arguments
        // (empty record).
        self.exec_plan(connection, plan, in_transaction).await
    }

    /// Returns a new [`ExprContext`](stmt::ExprContext) for a specific target.
    fn expr_cx_for<'a>(&'a self, target: impl stmt::IntoExprTarget<'a>) -> stmt::ExprContext<'a> {
        stmt::ExprContext::new_with_target(&self.schema, target)
    }
}
