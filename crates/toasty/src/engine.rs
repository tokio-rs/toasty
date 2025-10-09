mod plan;
use plan::Plan;

mod eval;
mod exec;
mod index;
mod kv;
mod planner;
mod simplify;
mod ty;
mod verify;

use crate::Result;
use std::sync::Arc;
use toasty_core::{
    driver::Capability,
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

        // Translate the optimized statement into a series of driver operations.
        let plan = self.plan(stmt)?;

        // The plan is called once (single entry record stream) with no arguments
        // (empty record).
        self.exec_plan(&plan.pipeline, plan.vars).await
    }

    /// Returns a new ExprContext
    fn expr_cx(&self) -> stmt::ExprContext<'_> {
        stmt::ExprContext::new(&self.schema)
    }

    /// Returns a new ExprContext for a specific target
    fn expr_cx_for<'a>(&'a self, target: impl stmt::IntoExprTarget<'a>) -> stmt::ExprContext<'a> {
        stmt::ExprContext::new_with_target(&self.schema, target)
    }
}
