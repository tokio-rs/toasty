// Statement effect classification.  The consumer (pool retry wrapper)
// lands in a follow-up PR per `docs/dev/design/retry-safe-recovery.md`;
// allow dead code until then.
#[allow(dead_code)]
pub(crate) mod effect;
pub(crate) mod eval;
pub(crate) mod exec;

mod bind;
#[cfg(test)]
pub(crate) mod test_util;

mod fold;
mod hir;
use hir::HirStatement;

mod index;
mod legalize;
mod lower;
mod mir;
mod plan;
mod select_item;
pub(crate) use select_item::{SelectItem, SelectItems};
mod simplify;
mod ty;
mod upsert;
mod verify;

use crate::Result;
use std::sync::Arc;
use toasty_core::{
    Connection, Schema,
    driver::{Capability, operation::RawSql},
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
/// 4. **Execution.** Run actions against the database driver. Each
///    driver-bound statement is legalized for the target backend and its
///    bind parameters extracted ([`prepare_for_driver`](Self::prepare_for_driver))
///    immediately before it crosses to the driver.
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
        mut stmt: Statement,
        in_transaction: bool,
    ) -> Result<toasty_core::driver::ExecResponse> {
        upsert::apply_defaults(&mut stmt)?;
        self.verify(&stmt)?;

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

    /// Executes user-authored SQL through the driver SQL path.
    pub(crate) async fn exec_raw_sql(
        &self,
        connection: &mut dyn Connection,
        raw: RawSql,
    ) -> Result<toasty_core::driver::ExecResponse> {
        if !self.capability.sql {
            return Err(toasty_core::Error::unsupported_feature(format!(
                "{} does not support raw SQL",
                self.capability.driver_name
            )));
        }

        connection.exec(&self.schema, raw.into()).await
    }

    /// Returns a new [`ExprContext`](stmt::ExprContext) for a specific target.
    fn expr_cx_for<'a>(&'a self, target: impl stmt::IntoExprTarget<'a>) -> stmt::ExprContext<'a> {
        stmt::ExprContext::new_with_target(&self.schema, target)
    }
}
