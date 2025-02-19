use super::*;
use app::{FieldTy, Model};

// Strategy:
// * Create a batch of queries to operate atomically.
// * Queries might mix `insert`, `update`, and `delete`
// * Since Update may insert, it could trigger the full insertion planning path.

impl Planner<'_> {
    // If the update statement requested the result to be returned, then this
    // method returns the var in which it will be stored.
    pub(super) fn plan_update(&mut self, mut stmt: stmt::Update) -> Option<plan::VarId> {
        self.simplify_stmt_update(&mut stmt);

        let model = self.model(stmt.target.as_model_id());

        // Make sure the update statement isn't empty
        assert!(
            !stmt.assignments.is_empty(),
            "update must update some columns"
        );

        let scope = stmt.selection();

        // Handle any relation updates
        for (i, field) in model.fields.iter().enumerate() {
            if !stmt.assignments.contains(i) {
                continue;
            }

            let Some(assignment) = stmt.assignments.get_mut(&i) else {
                continue;
            };

            match assignment.op {
                stmt::AssignmentOp::Set => assert!(!field.ty.is_has_many(), "TODO"),
                stmt::AssignmentOp::Insert => assert!(field.ty.is_has_many(), "TODO"),
                stmt::AssignmentOp::Remove => assert!(field.ty.is_has_many(), "TODO"),
            }

            self.plan_mut_relation_field(field, assignment.op, &mut assignment.expr, &scope, false);

            // Map the belongs_to statement to the foreign key fields
            if let FieldTy::BelongsTo(belongs_to) = &field.ty {
                let stmt::Expr::Value(value) = stmt.assignments.take(i).expr else {
                    todo!()
                };

                match value {
                    stmt::Value::Null => {
                        for fk_field in &belongs_to.foreign_key.fields {
                            stmt.assignments.set(fk_field.source, stmt::Expr::null());
                        }
                    }
                    value => {
                        let [fk_field] = &belongs_to.foreign_key.fields[..] else {
                            todo!("composite keys")
                        };

                        stmt.assignments.set(fk_field.source, value);
                    }
                }
            } else if field.is_relation() {
                stmt.assignments.unset(i);
            }
        }

        if stmt.assignments.is_empty() {
            stmt.returning.as_ref()?;

            let value = stmt::Value::empty_sparse_record();
            return Some(self.set_var(
                vec![value],
                stmt::Type::list(stmt::Type::empty_sparse_record()),
            ));
        }

        if !self.capability.is_sql() {
            // Subqueries are planned before lowering
            self.plan_subqueries(&mut stmt);
        }

        self.lower_stmt_update(model, &mut stmt);

        if self.capability.is_sql() {
            self.plan_update_sql(stmt)
        } else {
            self.plan_update_kv(model, stmt)
        }
    }

    fn plan_update_sql(&mut self, mut stmt: stmt::Update) -> Option<plan::VarId> {
        let output = self
            .partition_maybe_returning(&mut stmt.returning)
            .map(|project| plan::Output {
                var: self
                    .var_table
                    .register_var(stmt::Type::list(project.ret.clone())),
                project,
            });

        let output_var = output.as_ref().map(|o| o.var);

        self.push_action(plan::ExecStatement {
            output,
            input: None,
            stmt: stmt.into(),
        });

        output_var
    }

    fn plan_update_kv(&mut self, model: &Model, mut stmt: stmt::Update) -> Option<plan::VarId> {
        let table = self.schema.table_for(model);

        // Figure out which index to use for the query
        // let input = self.extract_input(&mut filter, &[], true);

        let mut index_plan =
            self.plan_index_path2(table, stmt.filter.as_ref().expect("no filter specified"));

        assert!(!stmt.assignments.is_empty());

        let output = self
            .partition_maybe_returning(&mut stmt.returning)
            .map(|project| plan::Output {
                var: self
                    .var_table
                    .register_var(stmt::Type::list(project.ret.clone())),
                project,
            });

        let output_var = output.as_ref().map(|o| o.var);

        if index_plan.index.primary_key {
            let Some(key) = self.try_build_key_filter(index_plan.index, &index_plan.index_filter)
            else {
                todo!("index_filter={:#?}", index_plan.index_filter);
            };

            self.push_write_action(plan::UpdateByKey {
                input: None,
                output,
                table: table.id,
                keys: key,
                assignments: stmt.assignments,
                filter: index_plan.result_filter,
                condition: stmt.condition,
            });

            output_var
        } else {
            debug_assert!(index_plan.post_filter.is_none());
            debug_assert!(!stmt.assignments.is_empty());
            assert!(stmt.condition.is_none());

            // Find existing associations so we can delete them
            // TODO: this should be atomic
            let update_by_key_input = self.plan_find_pk_by_index(&mut index_plan, None);
            let keys = eval::Func::identity(update_by_key_input.project.ret.clone());

            self.push_write_action(plan::UpdateByKey {
                input: Some(update_by_key_input),
                output,
                table: table.id,
                keys,
                assignments: stmt.assignments,
                filter: index_plan.result_filter,
                condition: None,
            });

            output_var
        }
    }
}
