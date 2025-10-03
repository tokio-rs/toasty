use super::{eval, plan, Exec, Result};
use std::collections::HashMap;
use toasty_core::stmt;
use toasty_core::stmt::ValueStream;

impl Exec<'_> {
    pub(super) async fn action_nested_merge(&mut self, action: &plan::NestedMerge) -> Result<()> {
        todo!()
    }
}
