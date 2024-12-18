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

        self.lower_stmt_delete(model, &mut stmt);

        if self.capability.is_sql() {
            self.plan_delete_sql(model, stmt);
        } else {
            self.plan_subqueries(&mut stmt);
            self.plan_delete_kv(model, stmt);
        }
    }

    fn plan_delete_sql(&mut self, model: &Model, mut stmt: stmt::Delete) {
        self.push_action(plan::QuerySql {
            output: None,
            input: None,
            stmt: stmt.into(),
        });
    }

    fn plan_delete_kv(&mut self, model: &Model, mut stmt: stmt::Delete) {
        let table = self.schema.table(model.lowering.table);

        // Figure out which index to use for the query
        let index_plan = self.plan_index_path2(table, &stmt.filter);

        if index_plan.index.primary_key {
            if let Some(keys) =
                self.try_build_key_filter(index_plan.index, &index_plan.index_filter)
            {
                self.push_write_action(plan::DeleteByKey {
                    input: None,
                    table: model.lowering.table,
                    keys,
                    filter: index_plan.result_filter,
                });
            } else {
                todo!(
                    "subqueries={:#?}; index_plan.filter={:#?}",
                    self.subqueries,
                    index_plan.index_filter,
                );
            };
        } else {
            /*
            assert!(index_plan.post_filter.is_none());

            let pk_by_index_out = self.var_table.register_var();
            self.push_action(plan::FindPkByIndex {
                input,
                output: pk_by_index_out,
                table: table.id,
                index: index_plan.index.lowering.index,
                filter: index_filter,
            });

            // TODO: include a deletion condition that ensures the index fields
            // match the query (i.e. the record is still included by the index
            // above and not concurrently updated since the index was query).
            self.push_write_action(plan::DeleteByKey {
                input: vec![plan::Input::from_var(pk_by_index_out)],
                table: table.id,
                keys: eval::Expr::project(eval::Expr::arg(0), [0]),
                filter: index_plan.result_filter.map(|mut expr| {
                    self.lower_expr2(model, &mut expr);
                    expr
                }),
            });
            */
            todo!()
        }
    }
}
