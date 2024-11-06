use super::*;

#[derive(Debug, Default)]
pub(super) struct Context<'stmt> {
    /// If the statement references any arguments (`stmt::ExprArg`), this
    /// informs the planner how to access those arguments.
    input: Vec<plan::Input<'stmt>>,
}

impl<'stmt> Planner<'_, 'stmt> {
    /// Plan a select statement, returning the variable ID where the output will
    /// be stored.
    pub(super) fn plan_select(&mut self, stmt: stmt::Query<'stmt>) -> plan::VarId {
        self.plan_select2(&Context::default(), stmt)
    }

    fn plan_select2(&mut self, cx: &Context<'stmt>, mut stmt: stmt::Query<'stmt>) -> plan::VarId {
        self.simplify_stmt_query(&mut stmt);
        self.plan_simplified_select(cx, stmt)
    }

    pub(super) fn plan_simplified_select(
        &mut self,
        cx: &Context<'stmt>,
        stmt: stmt::Query<'stmt>,
    ) -> plan::VarId {
        // TODO: don't clone?
        let source_model = stmt.body.as_select().source.as_model().clone();
        let model = self.schema.model(source_model.model);

        let ret = if self.capability.is_sql() {
            self.plan_select_sql(cx, model, stmt)
        } else {
            self.plan_select_kv(cx, model, stmt)
        };

        if !source_model.include.is_empty() {
            // For now, the full model must be selected
            // assert!(stmt.returning.is_star());
            todo!()
        }

        for include in &source_model.include {
            // self.plan_select_include(stmt.source.as_model_id(), include, ret);
            todo!()
        }

        ret
    }

    fn plan_select_sql(
        &mut self,
        cx: &Context<'stmt>,
        model: &Model,
        mut stmt: stmt::Query<'stmt>,
    ) -> plan::VarId {
        let table = self.schema.table(model.lowering.table);

        let lowering = self.lower_stmt_query(table, model, &mut stmt);

        let output = self.var_table.register_var();

        self.push_action(plan::QuerySql {
            input: cx.input.clone(),
            output: Some(plan::QuerySqlOutput {
                var: output,
                project: lowering.project,
            }),
            stmt: stmt.into(),
        });

        output
    }

    fn plan_select_kv(
        &mut self,
        cx: &Context<'stmt>,
        model: &Model,
        stmt: stmt::Query<'stmt>,
    ) -> plan::VarId {
        let table = self.schema.table(model.lowering.table);
        /*

        // TODO: don't clone
        let filter = stmt.filter.clone();

        let mut index_plan = self.plan_index_path2(model, &filter);

        let mut index_filter = index_plan.index_filter;
        let index = self.schema.index(index_plan.index.lowering.index);
        self.lower_index_filter(table, model, index_plan.index, &mut index_filter);

        if let Some(result_filter) = &mut index_plan.result_filter {
            self.lower_expr2(model, result_filter);
        }

        if index_plan.index.primary_key {
            // Is the index filter a set of keys
            if let Some(keys) = self.try_build_key_filter(index, &index_filter) {
                assert!(index_plan.post_filter.is_none());

                let output = self.var_table.register_var();

                self.push_action(plan::GetByKey {
                    input: cx.input.clone(),
                    output,
                    table: table.id,
                    columns: model.lowering.columns.clone(),
                    keys,
                    project,
                    post_filter: index_plan.result_filter.map(eval::Expr::from_stmt),
                });

                output
            } else {
                assert!(stmt.returning.is_star());
                assert!(cx.input.is_empty());

                let output = self.var_table.register_var();

                self.push_action(plan::QueryPk {
                    output,
                    table: table.id,
                    columns: model.lowering.columns.clone(),
                    pk_filter: sql::Expr::from_stmt(self.schema, table.id, index_filter),
                    project,
                    filter: index_plan
                        .result_filter
                        .map(|stmt| sql::Expr::from_stmt(self.schema, table.id, stmt)),
                    post_filter: index_plan.post_filter.map(eval::Expr::from_stmt),
                });

                output
            }
        } else {
            assert!(index_plan.post_filter.is_none());

            let filter = sql::Expr::from_stmt(self.schema, table.id, index_filter);

            let pk_by_index_out = self.var_table.register_var();
            self.push_action(plan::FindPkByIndex {
                input: cx.input.clone(),
                output: pk_by_index_out,
                table: table.id,
                index: index_plan.index.lowering.index,
                filter,
            });

            let get_by_key_out = self.var_table.register_var();

            self.push_action(plan::GetByKey {
                input: vec![plan::Input::from_var(pk_by_index_out)],
                output: get_by_key_out,
                table: table.id,
                keys: eval::Expr::project([0]),
                columns: model.lowering.columns.clone(),
                project,
                post_filter: index_plan.result_filter.map(eval::Expr::from_stmt),
            });

            get_by_key_out
        }
        */
        todo!()
    }

    fn plan_select_include(&mut self, base: ModelId, path: &stmt::Path, input: plan::VarId) {
        // TODO: move this into verifier
        assert_eq!(base, path.root);

        let [step] = &path[..] else { todo!() };

        let model = self.model(base);
        let field = &model.fields[step.into_usize()];

        match &field.ty {
            FieldTy::HasMany(rel) => {
                let pair = rel.pair(self.schema);

                let [fk_field] = &pair.foreign_key.fields[..] else {
                    todo!("composite key")
                };

                let cx = Context {
                    input: vec![plan::Input::project_var_ref(
                        input,
                        eval::Expr::map(
                            eval::Expr::arg(0),
                            eval::Expr::project(eval::Expr::arg(0), fk_field.target),
                        ),
                    )],
                };

                let filter = stmt::Expr::in_list(fk_field.source, stmt::Expr::arg(0));
                let out = self.plan_select2(&cx, stmt::Query::filter(rel.target, filter));

                // Associate target records with the source
                self.push_action(plan::Associate {
                    source: input,
                    target: out,
                    field: field.id,
                });
            }
            FieldTy::BelongsTo(rel) => {
                let [fk_field] = &rel.foreign_key.fields[..] else {
                    todo!("composite key")
                };

                let cx = Context {
                    input: vec![plan::Input::project_var_ref(
                        input,
                        eval::Expr::map(
                            eval::Expr::arg(0),
                            eval::Expr::project(eval::Expr::arg(0), fk_field.source),
                        ),
                    )],
                };

                let filter = stmt::Expr::in_list(fk_field.target, stmt::Expr::arg(0));
                let out = self.plan_select2(&cx, stmt::Query::filter(rel.target, filter));

                // Associate target records with the source
                self.push_action(plan::Associate {
                    source: input,
                    target: out,
                    field: field.id,
                });
            }
            _ => todo!("field.ty={:#?}", field.ty),
        }
    }
}
