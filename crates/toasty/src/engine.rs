mod eval;
mod exec;

mod plan;
use plan::{Action, Plan};

mod planner;
mod simplify;
mod ty;
mod verify;

use crate::{Db, Result};

use toasty_core::{
    stmt::{self, Statement, ValueStream},
    Schema,
};

pub(crate) async fn exec(db: &Db, stmt: Statement) -> Result<ValueStream> {
    if cfg!(debug_assertions) {
        verify::apply(&db.schema, &stmt);
    }

    // Translate the optimized statement into a series of driver operations.
    let plan = planner::apply(db.driver.capability(), &db.schema, stmt)?;

    // The plan is called once (single entry record stream) with no arguments
    // (empty record).
    exec::exec(db, &plan.pipeline, plan.vars).await
}
