use super::*;

// Strategy:
// * Create a batch of queries to operate atomically.
// * Queries might mix `insert`, `update`, and `delete`
// * Since Update may insert, it could trigger the full insertion planning path.

impl Planner<'_> {
    // If the update statement requested the result to be returned, then this
    // method returns the var in which it will be stored.
    pub(super) fn plan_update(&mut self, mut stmt: stmt::Update) -> Option<plan::VarId> {
        println!("update={stmt:#?}");
        self.simplify_stmt_update(&mut stmt);
        println!("simplified={stmt:#?}");

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
            if stmt.returning.is_none() {
                return None;
            }

            let value = stmt::Value::empty_sparse_record();
            return Some(self.set_var(
                vec![value],
                stmt::Type::list(stmt::Type::empty_sparse_record()),
            ));
        }

        self.lower_stmt_update(model, &mut stmt);
        self.constantize_update_returning(&mut stmt);

        if self.capability.is_sql() {
            self.plan_update_sql(stmt)
        } else {
            self.plan_subqueries(&mut stmt);
            self.plan_update_kv(model, stmt)
        }
    }

    fn plan_update_sql(&mut self, mut stmt: stmt::Update) -> Option<plan::VarId> {
        let model = self.model(stmt.target.as_model_id());

        let output = self
            .partition_maybe_returning(&mut stmt.returning)
            .map(|mut project| plan::Output {
                var: self
                    .var_table
                    .register_var(stmt::Type::list(project.ret.clone())),
                project,
            });

        let output_var = output.as_ref().map(|o| o.var);

        self.push_action(plan::QuerySql {
            output,
            input: None,
            stmt: stmt.into(),
        });

        output_var
    }

    fn plan_update_kv(&mut self, model: &Model, mut stmt: stmt::Update) -> Option<plan::VarId> {
        let table = self.schema.table(model.lowering.table);

        // Figure out which index to use for the query
        // let input = self.extract_input(&mut filter, &[], true);

        let index_plan =
            self.plan_index_path2(table, stmt.filter.as_ref().expect("no filter specified"));

        assert!(!stmt.assignments.is_empty());

        let output = self
            .partition_maybe_returning(&mut stmt.returning)
            .map(|mut project| plan::Output {
                var: self
                    .var_table
                    .register_var(stmt::Type::list(project.ret.clone())),
                project,
            });

        let output_var = output.as_ref().map(|o| o.var);

        if index_plan.index.primary_key {
            let Some(key) = self.try_build_key_filter(&index_plan.index, &index_plan.index_filter)
            else {
                todo!("index_filter={:#?}", index_plan.index_filter);
            };

            self.push_write_action(plan::UpdateByKey {
                input: None,
                output,
                table: model.lowering.table,
                keys: key,
                assignments: stmt.assignments,
                filter: index_plan.result_filter,
                condition: stmt.condition,
            });

            output_var
        } else {
            todo!("stmt={stmt:#?}");
            /*
            debug_assert!(index_plan.post_filter.is_none());

            // Find existing associations so we can delete them
            // TODO: leverage select path
            // TODO: this should be atomic
            let pk_by_index_out = self.var_table.register_var();

            self.push_action(plan::FindPkByIndex {
                input,
                output: pk_by_index_out,
                table: table.id,
                index: index_plan.index.lowering.index,
                filter: sql::Expr::from_stmt(self.schema, table.id, index_filter),
            });

            let output = if stmt.returning {
                Some(self.var_table.register_var())
            } else {
                None
            };

            debug_assert!(!columns.is_empty());
            assert!(stmt.condition.is_none());

            let assignments = columns
                .into_iter()
                .zip(projected.into_iter())
                .map(|(column, stmt)| sql::Assignment {
                    target: column,
                    value: sql::Expr::from_stmt(self.schema, table.id, stmt),
                })
                .collect();

            self.push_write_action(plan::UpdateByKey {
                input: Some(pk_by_index_out),
                output,
                table: model.lowering.table,
                key: eval::Expr::identity(),
                assignments,
                filter: index_plan
                    .result_filter
                    .map(|stmt| sql::Expr::from_stmt(self.schema, table.id, stmt)),
                condition: None,
            });

            output
            */
        }
    }

    fn constantize_update_returning(&self, stmt: &mut stmt::Update) {
        // TODO: probably not worth doing because we have to issue the update
        // statement regardless
        /*
        if let Some(stmt::Returning::Expr(returning)) = &mut stmt.returning {
            stmt::visit_mut::for_each_expr_mut(returning, |expr| {
                let stmt::Expr::Column(expr_column) = expr else {
                    return;
                };
                let Some(stmt::Expr::Value(assignment)) =
                    stmt.assignments.get(expr_column.column.index)
                else {
                    return;
                };

                *expr = assignment.clone().into();
            });
        }
        */
    }
}
