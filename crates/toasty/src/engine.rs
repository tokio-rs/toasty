pub(crate) mod eval;
mod exec;

mod hir;
use hir::HirStatement;

mod index;
mod lower;
mod mir;
mod plan;
mod simplify;
use simplify::Simplify;
mod ty;
mod verify;

use crate::{
    db::{Pool, PoolConnection},
    Result,
};
use std::sync::Arc;
use toasty_core::{
    driver::{Capability, Driver},
    stmt::{self, Statement, ValueStream},
    Schema,
};

/// The query execution engine.
///
/// [`Engine`] orchestrates the multi-phase compilation pipeline that transforms
/// user queries into database operations. It owns the schema and driver, and
/// provides the main entry point ([`exec`](Self::exec)) for executing statements.
///
/// The execution pipeline follows this process:
///
/// 1. **Verification.** Validate statement structure (debug builds only).
/// 2. **Lowering.** Convert to HIR with dependency tracking.
/// 3. **Planning.** Build MIR operation graph.
/// 4. **Execution.** Run actions against the database driver.
#[derive(Debug, Clone)]
pub(crate) struct Engine {
    /// The schema being managed by this database instance.
    pub(crate) schema: Arc<Schema>,

    /// Handle to the connection pool.
    pub(crate) pool: Arc<Pool>,
}

impl Engine {
    /// Creates a new [`Engine`] with the given schema and driver.
    pub(crate) fn new(schema: Arc<Schema>, pool: Arc<Pool>) -> Engine {
        Engine { schema, pool }
    }

    /// Returns the driver's capabilities.
    pub(crate) fn capability(&self) -> &Capability {
        self.pool.capability()
    }

    /// Executes a statement and returns the result as a value stream.
    ///
    /// This is the main entry point for query execution. The statement passes
    /// through the full compilation pipeline (lowering → planning → execution)
    /// before being sent to the database driver.
    pub(crate) async fn exec(&self, stmt: Statement) -> Result<ValueStream> {
        self.exec_on(stmt, &mut self.pool.get().await?).await
    }

    /// Executes a statement on a given database connection. This is used in transactions,
    /// where multiple statements need be executed on the same connection.
    pub(crate) async fn exec_on(
        &self,
        stmt: Statement,
        connection: &mut PoolConnection,
    ) -> Result<ValueStream> {
        if cfg!(debug_assertions) {
            self.verify(&stmt);
        }

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

        // The plan is called once (single entry record stream) with no arguments
        // (empty record).
        self.exec_plan(plan, connection).await
    }

    /// Returns a new [`ExprContext`](stmt::ExprContext) for this engine's schema.
    fn expr_cx(&self) -> stmt::ExprContext<'_> {
        stmt::ExprContext::new(&self.schema)
    }

    /// Returns a new [`ExprContext`](stmt::ExprContext) for a specific target.
    fn expr_cx_for<'a>(&'a self, target: impl stmt::IntoExprTarget<'a>) -> stmt::ExprContext<'a> {
        stmt::ExprContext::new_with_target(&self.schema, target)
    }

    /// Returns the database driver this engine is using.
    pub(crate) fn driver(&self) -> &dyn Driver {
        self.pool.driver()
    }
}
