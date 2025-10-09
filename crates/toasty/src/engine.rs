mod eval;
mod exec;

mod index;

mod plan;
use std::sync::Arc;

use plan::Plan;

mod planner;
mod simplify;
mod verify;

use crate::Result;
use toasty_core::{
    driver::Capability,
    stmt::{Statement, ValueStream},
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
}
