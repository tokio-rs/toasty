mod eval;
mod exec;

mod plan;
use plan::Plan;

mod planner;
mod simplify;
mod verify;

use crate::{DbInner, Result};
use toasty_core::stmt::{Statement, ValueStream};

pub(crate) async fn exec(db: &DbInner, stmt: Statement) -> Result<ValueStream> {
    if cfg!(debug_assertions) {
        verify::apply(&db.schema, &stmt);
    }

    // Translate the optimized statement into a series of driver operations.
    let plan = planner::apply(db.driver.capability(), &db.schema, stmt)?;
    println!("plan={plan:#?}");

    // The plan is called once (single entry record stream) with no arguments
    // (empty record).
    exec::exec(db, &plan.pipeline, plan.vars).await
}
