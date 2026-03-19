use crate::{
    engine::{
        eval,
        exec::{Action, Exec, Output, VarId},
    },
    Result,
};
use toasty_core::{
    driver::{operation, Rows},
    schema::db::TableId,
    stmt::{self, ValueStream},
};

#[derive(Debug, Clone)]
pub(crate) struct UpdateByKey {
    /// If specified, use the input to generate the list of keys to update
    pub input: VarId,

    /// Where to store the result of the update
    pub output: Output,

    /// Which table to update
    pub table: TableId,

    /// Assignments
    pub assignments: stmt::Assignments,

    /// Only update keys that match the filter
    pub filter: Option<stmt::Expr>,

    /// Fail the update if the condition is not met
    pub condition: Option<stmt::Expr>,

    /// Guard evaluated before issuing the operation. When present and the
    /// expression evaluates to false, the operation is skipped entirely.
    pub pre_filter: Option<eval::Func>,

    /// Input variables for `pre_filter` evaluation.
    pub pre_filter_inputs: Vec<VarId>,

    /// When `true` return the record being updated *after* the update. When
    /// `false`, just return the count of updated rows.
    pub returning: bool,
}

impl Exec<'_> {
    pub(super) async fn action_update_by_key(&mut self, action: &UpdateByKey) -> Result<()> {
        // Evaluate the pre-filter guard. When it evaluates to false the
        // operation is skipped entirely.
        if let Some(pre_filter) = &action.pre_filter {
            let mut inputs = Vec::with_capacity(action.pre_filter_inputs.len());
            for var_id in &action.pre_filter_inputs {
                let data = self.vars.load(*var_id).await?.collect_as_value().await?;
                inputs.push(data);
            }
            if !pre_filter.eval_bool(&inputs)? {
                let res = if action.returning {
                    Rows::value_stream(ValueStream::default())
                } else {
                    Rows::Count(0)
                };
                self.vars
                    .store(action.output.var, action.output.num_uses, res);
                return Ok(());
            }
        }

        let keys = self
            .vars
            .load(action.input)
            .await?
            .collect_as_value()
            .await?
            .unwrap_list();

        let res = if keys.is_empty() {
            if action.returning {
                Rows::value_stream(ValueStream::default())
            } else {
                Rows::Count(0)
            }
        } else {
            let op = operation::UpdateByKey {
                table: action.table,
                keys,
                assignments: action.assignments.clone(),
                filter: action.filter.clone(),
                condition: action.condition.clone(),
                returning: action.returning,
            };

            let res = self.connection.exec(&self.engine.schema, op.into()).await?;

            debug_assert_eq!(!res.rows.is_count(), action.returning);

            res.rows
        };

        self.vars
            .store(action.output.var, action.output.num_uses, res);

        Ok(())
    }
}

impl From<UpdateByKey> for Action {
    fn from(src: UpdateByKey) -> Self {
        Self::UpdateByKey(src)
    }
}
