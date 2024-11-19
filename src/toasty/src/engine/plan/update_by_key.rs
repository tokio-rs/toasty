use super::*;
use crate::{driver::*, schema::*};

#[derive(Debug, Clone)]
pub(crate) struct UpdateByKey {
    /// If specified, use the input to generate the list of keys to update
    pub input: Option<VarId>,

    /// Where to store the result of the update
    pub output: Option<VarId>,

    /// Which table to update
    pub table: TableId,

    /// Which key(s) to update
    pub key: eval::Expr,

    /// Assignments
    pub assignments: stmt::Assignments<'static>,

    /// Only update keys that match the filter
    pub filter: Option<stmt::Expr<'static>>,

    pub condition: Option<stmt::Expr<'static>>,
}

impl UpdateByKey {
    pub(crate) fn apply(&self) -> Result<operation::UpdateByKey> {
        debug_assert!(!self.assignments.is_empty(), "plan = {self:#?}");

        let keys = match self.key.eval_const() {
            stmt::Value::List(keys) => keys,
            key => vec![key],
        };

        Ok(operation::UpdateByKey {
            table: self.table,
            keys,
            assignments: self.assignments.clone(),
            filter: self.filter.clone(),
            condition: self.condition.clone(),
            returning: self.output.is_some(),
        })
    }

    pub(crate) async fn apply_with_input(
        &self,
        mut input: ValueStream,
    ) -> Result<operation::UpdateByKey> {
        debug_assert!(!self.assignments.is_empty(), "plan = {self:#?}");

        let mut keys = vec![];

        while let Some(res) = input.next().await {
            keys.push(if self.key.is_identity() {
                res?
            } else {
                todo!()
            });
        }

        Ok(operation::UpdateByKey {
            table: self.table,
            keys,
            assignments: self.assignments.clone(),
            filter: self.filter.clone(),
            condition: self.condition.clone(),
            returning: self.output.is_some(),
        })
    }
}

impl From<UpdateByKey> for Action {
    fn from(src: UpdateByKey) -> Action {
        Action::UpdateByKey(src)
    }
}
