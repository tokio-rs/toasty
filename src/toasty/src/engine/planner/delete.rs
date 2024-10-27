use super::*;

impl<'stmt> Planner<'_, 'stmt> {
    pub(super) fn plan_delete(&mut self, mut stmt: stmt::Delete<'stmt>) {
        self.simplify_stmt_delete(&mut stmt);

        let model = self.model(stmt.selection.body.as_select().source.as_model_id());

        // Handle any cascading deletes
        for field in model.fields.iter() {
            if let Some(rel) = field.ty.as_has_one() {
                // HAX: unify w/ relation planner
                if self.relations.last().copied() != Some(rel.pair) {
                    self.relations.push(field.id);
                    self.plan_mut_has_one_nullify(rel, &stmt.selection);
                    self.relations.pop();
                }
            } else if let Some(rel) = field.ty.as_has_many() {
                let pair = self.schema.field(rel.pair);

                // TODO: can this be unified with update?
                let query = stmt::Query::filter(
                    rel.target,
                    stmt::Expr::in_subquery(rel.pair, stmt.selection.clone()),
                );

                if pair.nullable {
                    let mut update = query.update(self.schema);
                    update.set(pair.id, stmt::Value::Null);

                    self.plan_update(update);
                } else {
                    self.plan_delete(query.delete());
                }
            }
        }

        // Plan subqueries
        self.plan_subqueries(&mut stmt);

        if self.capability.is_sql() {
            self.plan_delete_sql(model, stmt);
        } else {
            self.plan_delete_kv(model, stmt);
        }
    }

    fn plan_delete_sql(&mut self, model: &Model, stmt: stmt::Delete<'stmt>) {
        let mut filter = stmt.selection.body.as_select().filter.clone();

        self.lower_expr2(model, &mut filter);

        let sql = sql::Statement::delete(self.schema, model.lowering.table, filter);

        self.push_action(plan::QuerySql {
            output: None,
            input: vec![],
            sql,
        });
    }

    fn plan_delete_kv(&mut self, model: &Model, mut stmt: stmt::Delete<'stmt>) {
        let table = self.schema.table(model.lowering.table);

        let filter = &mut stmt.selection.body.as_select_mut().filter;
        let input = self.extract_input(filter, &[], true);

        // Figure out which index to use for the query
        let index_plan = self.plan_index_path2(model, filter);
        let mut index_filter = index_plan.index_filter;
        let index = self.schema.index(index_plan.index.lowering.index);
        self.lower_index_filter(table, model, index_plan.index, &mut index_filter);

        if index_plan.index.primary_key {
            if let Some(keys) = self.try_build_key_filter(index, &index_filter) {
                let filter = index_plan.result_filter.clone().map(|mut stmt| {
                    self.lower_expr2(model, &mut stmt);
                    sql::Expr::from_stmt(self.schema, table.id, stmt)
                });

                self.push_write_action(plan::DeleteByKey {
                    input,
                    table: model.lowering.table,
                    keys,
                    filter,
                });
            } else {
                todo!(
                    "subqueries={:#?}; index_plan.filter={:#?}",
                    self.subqueries,
                    index_filter
                );
            };
        } else {
            assert!(index_plan.post_filter.is_none());

            let pk_by_index_out = self.var_table.register_var();
            self.push_action(plan::FindPkByIndex {
                input,
                output: pk_by_index_out,
                table: table.id,
                index: index_plan.index.lowering.index,
                filter: sql::Expr::from_stmt(self.schema, table.id, index_filter),
            });

            // TODO: include a deletion condition that ensures the index fields
            // match the query (i.e. the record is still included by the index
            // above and not concurrently updated since the index was query).
            self.push_write_action(plan::DeleteByKey {
                input: vec![plan::Input::from_var(pk_by_index_out)],
                table: table.id,
                keys: eval::Expr::project([0]),
                filter: index_plan
                    .result_filter
                    .map(|stmt| sql::Expr::from_stmt(self.schema, table.id, stmt)),
            });
        }
    }
}
