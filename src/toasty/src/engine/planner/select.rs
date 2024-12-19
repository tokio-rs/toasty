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
        mut stmt: stmt::Query,
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

        self.lower_stmt_query(model, &mut stmt);

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
            output: Some(plan::Output {
                var: output,
                project,
            }),
            stmt: stmt.into(),
        });

        output
    }

    fn plan_select_kv(
        &mut self,
        cx: &Context,
        model: &Model,
        mut stmt: stmt::Query,
    ) -> plan::VarId {
        println!("stmt={stmt:#?}");
        let table = self.schema.table(model.lowering.table);

        let mut index_plan = match &*stmt.body {
            stmt::ExprSet::Select(query) => self.plan_index_path2(table, &query.filter),
            _ => todo!("stmt={stmt:#?}"),
        };

        let keys = if index_plan.index.primary_key {
            self.try_build_key_filter(index_plan.index, &index_plan.index_filter)
        } else {
            None
        };

        // TODO: clean all of this up!
        let result_post_filter = if keys.is_some() {
            // Because we are querying by key, the result filter must be
            // applied in-memory. TODO: some DBs might support filtering in
            // the DB.
            index_plan.result_filter.clone().map(|expr| {
                struct Columns<'a>(&'a mut Vec<stmt::Expr>);

                impl eval::Convert for Columns<'_> {
                    fn convert_expr_column(&mut self, stmt: stmt::ExprColumn) -> eval::Expr {
                        let index = self.0.iter().position(|expr| match expr {
                            stmt::Expr::Column(expr) => expr.column == stmt.column,
                            _ => false,
                        });

                        let index = if let Some(index) = index {
                            index
                        } else {
                            let index = self.0.len();
                            self.0.push(stmt::Expr::column(stmt.column));
                            index
                        };

                        eval::Expr::project(eval::Expr::arg(0), [index])
                    }
                }

                let convert = Columns(
                    &mut stmt
                        .body
                        .as_select_mut()
                        .returning
                        .as_expr_mut()
                        .as_record_mut()
                        .fields,
                );

                eval::Expr::try_convert_from_stmt(expr, convert).unwrap()
            })
        } else {
            None
        };

        let input = if cx.input.is_empty() {
            None
        } else {
            self.partition_query_input(&mut stmt, &cx.input)
        };

        let project = self.partition_returning(&mut stmt.body.as_select_mut().returning);
        let output = self
            .var_table
            .register_var(stmt::Type::list(project.ret.clone()));

        let mut columns = match &stmt.body.as_select().returning {
            stmt::Returning::Expr(stmt::Expr::Record(expr_record)) => expr_record
                .fields
                .iter()
                .map(|expr| match expr {
                    stmt::Expr::Column(expr) => expr.column,
                    _ => todo!("stmt={stmt:#?}"),
                })
                .collect(),
            _ => todo!("stmt={stmt:#?}"),
        };

        if index_plan.index.primary_key {
            // Is the index filter a set of keys
            if let Some(keys) = keys {
                assert!(index_plan.post_filter.is_none());

                let result_filter =
                    result_post_filter.map(|expr| eval::Func::new(project.args.clone(), expr));

                self.push_action(plan::GetByKey {
                    input,
                    output: plan::Output {
                        var: output,
                        project,
                    },
                    table: table.id,
                    columns,
                    keys,
                    post_filter: result_filter,
                });

                output
            } else {
                assert!(cx.input.is_empty());

                let post_filter = index_plan
                    .post_filter
                    .map(|expr| eval::Func::new(project.args.clone(), eval::Expr::from_stmt(expr)));

                self.push_action(plan::QueryPk {
                    output: plan::Output {
                        var: output,
                        project,
                    },
                    table: table.id,
                    columns: model.lowering.columns.clone(),
                    pk_filter: index_plan.index_filter,
                    filter: index_plan.result_filter,
                    post_filter,
                });

                output
            }
        } else {
            assert!(index_plan.post_filter.is_none());

            let get_by_key_input = self.plan_find_pk_by_index(&mut index_plan, input);

            let post_filter =
                result_post_filter.map(|expr| eval::Func::new(project.args.clone(), expr));

            let keys = eval::Func::identity(get_by_key_input.project.ret.clone());

            self.push_action(plan::GetByKey {
                input: Some(get_by_key_input),
                output: plan::Output {
                    var: output,
                    project,
                },
                table: table.id,
                keys,
                columns: model.lowering.columns.clone(),
                post_filter,
            });

            output
        }
    }

    fn plan_select_include(&mut self, base: ModelId, path: &stmt::Path, input: plan::VarId) {
        // TODO: move this into verifier
        assert_eq!(base, path.root);

        let [step] = &path.projection[..] else {
            todo!()
        };

        let model = self.model(base);
        let field = &model.fields[*step];

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
