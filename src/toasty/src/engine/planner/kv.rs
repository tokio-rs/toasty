//! Utilities for planning key-value operations.

use super::*;

impl Planner<'_> {
    pub(super) fn plan_find_pk_by_index(
        &mut self,
        index_plan: &mut IndexPlan<'_>,
        input: Option<plan::Input>,
    ) -> plan::Input {
        let key_ty = self.index_key_ty(index_plan.index);
        let pk_by_index_out = self
            .var_table
            .register_var(stmt::Type::list(key_ty.clone()));

        // In this case, we have to flatten the returned record into a single value
        let project_key = if index_plan.index.columns.len() == 1 {
            let arg_ty = stmt::Type::Record(vec![self
                .schema
                .db
                .column(index_plan.index.columns[0].column)
                .ty
                .clone()]);

            eval::Func::from_stmt_unchecked(
                stmt::Expr::arg_project(0, [0]),
                vec![arg_ty],
                key_ty.clone(),
            )
        } else {
            eval::Func::identity(key_ty.clone())
        };

        self.push_action(plan::FindPkByIndex {
            input,
            output: plan::Output {
                var: pk_by_index_out,
                project: project_key,
            },
            table: index_plan.index.on,
            index: index_plan.index.id,
            filter: index_plan.index_filter.take(),
        });

        plan::Input::from_var(pk_by_index_out, stmt::Type::list(key_ty.clone()))
    }
}
