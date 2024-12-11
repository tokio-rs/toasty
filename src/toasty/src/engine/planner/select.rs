use super::*;

#[derive(Debug, Default)]
pub(super) struct Context {
    /// If the statement references any arguments (`stmt::ExprArg`), this
    /// informs the planner how to access those arguments.
    input: Vec<plan::InputSource>,
}

impl Planner<'_> {
    /// Plan a select statement, returning the variable ID where the output will
    /// be stored.
    pub(super) fn plan_select(&mut self, stmt: stmt::Query) -> plan::VarId {
        self.plan_select2(&Context::default(), stmt)
    }

    fn plan_select2(&mut self, cx: &Context, mut stmt: stmt::Query) -> plan::VarId {
        self.simplify_stmt_query(&mut stmt);
        self.plan_simplified_select(cx, stmt)
    }

    pub(super) fn plan_simplified_select(
        &mut self,
        cx: &Context,
        stmt: stmt::Query,
    ) -> plan::VarId {
        // TODO: don't clone?
        let source_model = stmt.body.as_select().source.as_model().clone();
        let model = self.schema.model(source_model.model);

        let source_model = match &*stmt.body {
            stmt::ExprSet::Select(select) => {
                match &select.source {
                    stmt::Source::Model(source_model) => {
                        if !source_model.include.is_empty() {
                            // For now, the full model must be selected
                            assert!(matches!(select.returning, stmt::Returning::Star));
                        }

                        source_model.clone()
                    }
                    _ => todo!(),
                }
            }
            _ => todo!(),
        };

        let ret = if self.capability.is_sql() {
            self.plan_select_sql(cx, model, stmt)
        } else {
            self.plan_select_kv(cx, model, stmt)
        };

        for include in &source_model.include {
            self.plan_select_include(source_model.model, include, ret);
        }

        ret
    }

    fn plan_select_sql(
        &mut self,
        cx: &Context,
        model: &Model,
        mut stmt: stmt::Query,
    ) -> plan::VarId {
        self.lower_stmt_query(model, &mut stmt);

        let input = if cx.input.is_empty() {
            None
        } else {
            self.partition_query_input(&mut stmt, &cx.input)
        };

        let project = self.partition_returning(&mut stmt.body.as_select_mut().returning);
        let output = self
            .var_table
            .register_var(stmt::Type::list(project.ret.clone()));

        if let Some(input) = &input {
            assert!(input.project.args[0].is_list(), "{input:#?}");
        }

        self.push_action(plan::QuerySql {
            input,
            output: Some(plan::QuerySqlOutput {
                var: output,
                project,
            }),
            stmt: stmt.into(),
        });

        output
    }

    fn plan_select_kv(&mut self, cx: &Context, model: &Model, stmt: stmt::Query) -> plan::VarId {
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
                    input: vec![plan::InputSource::Ref(input)],
                };

                let filter = stmt::Expr::in_list(
                    fk_field.source,
                    stmt::Expr::map(
                        stmt::Expr::arg(0),
                        stmt::Expr::project(stmt::Expr::arg(0), fk_field.target),
                    ),
                );

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
                    input: vec![plan::InputSource::Ref(input)],
                    /*
                    input: vec![plan::Input::project_var_ref(
                        input,
                        eval::Expr::project(eval::Expr::arg(0), fk_field.source),
                    )],
                    */
                };

                let filter = stmt::Expr::in_list(
                    fk_field.target,
                    stmt::Expr::map(
                        stmt::Expr::arg(0),
                        stmt::Expr::project(stmt::Expr::arg(0), fk_field.source),
                    ),
                );
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
