mod eval;
mod exec;

mod hir;
use hir::HirStatement;

mod index;
mod kv;
mod lower;
mod mir;
mod plan;
mod simplify;
use simplify::Simplify;
mod ty;
mod verify;

use crate::Result;
use std::sync::Arc;
use toasty_core::{
    driver::Capability,
    schema::db::Table,
    stmt::{self, Statement, ValueStream},
    Driver, Schema,
};

#[derive(Debug, Clone)]
pub(crate) struct Engine {
    /// Schema being managed by this DB instance.
    pub(crate) schema: Arc<Schema>,

    /// Handle to the underlying database driver.
    pub(crate) driver: Arc<dyn Driver>,
}

impl Engine {
    pub(crate) fn new(schema: Arc<Schema>, driver: Arc<dyn Driver>) -> Engine {
        Engine { schema, driver }
    }

    pub(crate) fn capability(&self) -> &Capability {
        self.driver.capability()
    }

    pub(crate) async fn exec(&self, stmt: Statement) -> Result<ValueStream> {
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
        self.exec_plan(plan).await
    }

    /// Returns a new ExprContext
    fn expr_cx(&self) -> stmt::ExprContext<'_> {
        stmt::ExprContext::new(&self.schema)
    }

    /// Returns a new ExprContext for a specific target
    fn expr_cx_for<'a>(&'a self, target: impl stmt::IntoExprTarget<'a>) -> stmt::ExprContext<'a> {
        stmt::ExprContext::new_with_target(&self.schema, target)
    }

    // TODO: where should this util go?
    fn resolve_table_for<'a>(&'a self, target: impl stmt::IntoExprTarget<'a>) -> &'a Table {
        self.expr_cx_for(target).target().as_table_unwrap()
    }
}
