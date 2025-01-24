use super::*;

impl Planner<'_> {
    pub(super) fn plan_delete(&mut self, mut stmt: stmt::Delete) {
        self.simplify_stmt_delete(&mut stmt);

        let model = self.model(stmt.from.as_model_id());
        let selection = stmt.selection();

        // Handle any cascading deletes
        for field in model.fields.iter() {
            if let Some(rel) = field.ty.as_has_one() {
                // HAX: unify w/ relation planner
                if self.relations.last().copied() != Some(rel.pair) {
                    self.relations.push(field.id);
                    self.plan_mut_has_one_nullify(rel, &selection);
                    self.relations.pop();
                }
            } else if let Some(rel) = field.ty.as_has_many() {
                let pair = self.schema.field(rel.pair);

                // TODO: can this be unified with update?
                let query = stmt::Query::filter(
                    rel.target,
                    stmt::Expr::in_subquery(rel.pair, selection.clone()),
                );

                if pair.nullable {
                    let mut update = query.update();
                    update.assignments.set(pair.id, stmt::Value::Null);

                    self.plan_update(update);
                } else {
                    self.plan_delete(query.delete());
                }
            }
        }

        if self.capability.is_sql() {
            self.plan_delete_sql(model, stmt);
        } else {
            self.plan_delete_kv(model, stmt);
        }
    }

    fn plan_delete_sql(&mut self, model: &app::Model, mut stmt: stmt::Delete) {
        self.lower_stmt_delete(model, &mut stmt);

        self.push_action(plan::ExecStatement {
            output: None,
            input: None,
            stmt: stmt.into(),
        });
    }

    fn plan_delete_kv(&mut self, model: &app::Model, mut stmt: stmt::Delete) {
        let table = self.schema.table_for(model);

        // Subqueries are planned before lowering
        let input_sources = self.plan_subqueries(&mut stmt);

        self.lower_stmt_delete(model, &mut stmt);

        let input = if input_sources.is_empty() {
            None
        } else {
            self.partition_stmt_delete_input(&mut stmt, &input_sources)
        };

        // Figure out which index to use for the query
        let mut index_plan = self.plan_index_path2(table, &stmt.filter);

        if index_plan.index.primary_key {
            if let Some(keys) =
                self.try_build_key_filter(index_plan.index, &index_plan.index_filter)
            {
                self.push_write_action(plan::DeleteByKey {
                    input,
                    table: table.id,
                    keys,
                    filter: index_plan.result_filter,
                });
            } else {
                todo!(
                    "index_plan.filter={:#?}; stmt={stmt:#?}",
                    index_plan.index_filter,
                );
            };
        } else {
            assert!(index_plan.post_filter.is_none());

            let delete_by_key_input = self.plan_find_pk_by_index(&mut index_plan, input);
            let keys = eval::Func::identity(delete_by_key_input.project.ret.clone());

            // TODO: include a deletion condition that ensures the index fields
            // match the query (i.e. the record is still included by the index
            // above and not concurrently updated since the index was query).
            self.push_write_action(plan::DeleteByKey {
                input: Some(delete_by_key_input),
                table: table.id,
                keys,
                filter: index_plan.result_filter,
            });
        }
    }
}
