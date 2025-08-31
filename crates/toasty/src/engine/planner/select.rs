use super::{eval, plan, Context, Planner, Result};
use crate::engine::typed::Typed;
use toasty_core::{
    schema::app::{FieldTy, Model, ModelId},
    stmt,
};

impl Planner<'_> {
    pub(super) fn plan_stmt_select(
        &mut self,
        cx: &Context,
        typed_stmt: Typed<stmt::Query>,
    ) -> Result<plan::VarId> {
        // Verify that the input type is always a List (as expected for queries)
        debug_assert!(
            typed_stmt.ty.is_list(),
            "Expected List type at start of plan_stmt_select, got: {:?}",
            typed_stmt.ty
        );

        // TODO: don't clone?
        let source_model = typed_stmt.value.body.as_select().source.as_model().clone();
        let model = self.schema.app.model(source_model.model);

        if !source_model.include.is_empty() {
            // For now, the full model must be selected
            assert!(matches!(
                typed_stmt.value.body.as_select().returning,
                stmt::Returning::Star
            ));
        }

        // Use the passed-in typed statement - no need to create our own
        let mut typed_stmt = typed_stmt;

        // Lowering will update both statement and type based on includes
        self.lower_stmt_query(model, &mut typed_stmt);

        // Verify that after lowering, there are no more model-level types
        debug_assert!(
            typed_stmt.ty.is_lowered(),
            "Type not properly lowered, still contains model-level types: {:?}",
            typed_stmt.ty
        );

        // partition_returning uses the correctly lowered type
        // After lowering, typed_stmt.ty should be List(Record(...))
        // Extract the inner record type for the projection function
        let record_type = match &typed_stmt.ty {
            stmt::Type::List(inner) => (**inner).clone(),
            _ => panic!(
                "Expected List type after lowering, got: {:?}",
                typed_stmt.ty
            ),
        };

        let project = self.partition_returning(
            &mut typed_stmt.value.body.as_select_mut().returning,
            record_type,
        );

        // Step 4: Register variable with the correct type (the full List type)
        let output = self.var_table.register_var(typed_stmt.ty.clone());

        // If the filter expression is false, then the result will be empty.
        if let stmt::ExprSet::Select(select) = &typed_stmt.value.body {
            if select.filter.is_false() {
                self.push_action(plan::SetVar {
                    var: output,
                    value: vec![],
                });
                return Ok(output);
            }
        }

        let ret = if self.capability.sql {
            self.plan_select_sql(cx, output, project, typed_stmt.value)
        } else {
            self.plan_select_kv(cx, model, output, project, typed_stmt.value)
        };

        for include in &source_model.include {
            self.plan_select_include(source_model.model, include, ret)?;
        }

        Ok(ret)
    }

    fn plan_select_sql(
        &mut self,
        cx: &Context,
        output: plan::VarId,
        project: eval::Func,
        mut stmt: stmt::Query,
    ) -> plan::VarId {
        self.rewrite_offset_after_as_filter(&mut stmt);

        let input = if cx.input.is_empty() {
            None
        } else {
            self.partition_stmt_query_input(&mut stmt, &cx.input)
        };

        if let Some(input) = &input {
            assert!(input.project.args[0].is_list(), "{input:#?}");
        }

        self.push_action(plan::ExecStatement {
            input,
            output: Some(plan::Output {
                var: output,
                project,
            }),
            stmt: stmt.into(),
            conditional_update_with_no_returning: false,
        });

        output
    }

    fn plan_select_kv(
        &mut self,
        cx: &Context,
        model: &Model,
        output: plan::VarId,
        project: eval::Func,
        mut stmt: stmt::Query,
    ) -> plan::VarId {
        assert!(stmt.order_by.is_none(), "TODO: implement ordering for KV");
        assert!(stmt.limit.is_none(), "TODO: implement limit for KV");

        let table = self.schema.table_for(model);

        // Extract parts of the query that must be executed in-memory.
        let input = if cx.input.is_empty() {
            None
        } else {
            self.partition_stmt_query_input(&mut stmt, &cx.input)
        };

        let mut index_plan = match &stmt.body {
            stmt::ExprSet::Select(query) => self.plan_index_path2(table, &query.filter),
            _ => todo!("stmt={stmt:#?}"),
        };

        let keys = if index_plan.index.primary_key {
            self.try_build_key_filter(index_plan.index, &index_plan.index_filter)
        } else {
            None
        };

        if keys.is_some() {
            // Because we are querying by key, the result filter must be
            // applied in-memory. TODO: some DBs might support filtering in
            // the DB.
            if let Some(filter) = &index_plan.result_filter {
                let returning = stmt
                    .body
                    .as_select_mut()
                    .returning
                    .as_expr_mut()
                    .as_record_mut();

                stmt::visit::for_each_expr(filter, |filter_expr| {
                    if let stmt::Expr::Column(filter_expr) = filter_expr {
                        let contains = returning.fields.iter().any(|e| match e {
                            stmt::Expr::Column(e) => e == filter_expr,
                            _ => false,
                        });

                        if !contains {
                            todo!("returning types won't like up with projection");
                            /*
                            returning
                                .fields
                                .push(stmt::Expr::column(filter_expr.column));
                            */
                        }
                    }
                });
            }
        }

        let columns = match &stmt.body.as_select().returning {
            stmt::Returning::Expr(stmt::Expr::Record(expr_record)) => expr_record
                .fields
                .iter()
                .map(|expr| match expr {
                    stmt::Expr::Column(expr) => {
                        expr.try_to_column_id().expect("not referencing column")
                    }
                    _ => todo!("stmt={stmt:#?}"),
                })
                .collect(),
            _ => todo!("stmt={stmt:#?}"),
        };

        // TODO: clean all of this up!
        let result_post_filter = if !index_plan.index.primary_key || keys.is_some() {
            index_plan.result_filter.clone().map(|expr| {
                struct Columns<'a>(&'a mut Vec<stmt::Expr>);

                impl eval::Convert for Columns<'_> {
                    fn convert_expr_column(
                        &mut self,
                        stmt: &stmt::ExprColumn,
                    ) -> Option<stmt::Expr> {
                        let index = self
                            .0
                            .iter()
                            .position(|expr| match expr {
                                stmt::Expr::Column(expr) => expr == stmt,
                                _ => false,
                            })
                            .unwrap();

                        Some(stmt::Expr::project(stmt::Expr::arg(0), [index]))
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

                eval::Func::try_convert_from_stmt(expr, project.args.clone(), convert).unwrap()
            })
        } else {
            None
        };

        if index_plan.index.primary_key {
            // Is the index filter a set of keys
            if let Some(keys) = keys {
                assert!(index_plan.post_filter.is_none());

                self.push_action(plan::GetByKey {
                    input,
                    output: plan::Output {
                        var: output,
                        project,
                    },
                    table: table.id,
                    columns,
                    keys,
                    post_filter: result_post_filter,
                });

                output
            } else {
                assert!(cx.input.is_empty());

                let post_filter = index_plan.post_filter.map(|expr| {
                    struct Columns<'a>(&'a mut Vec<stmt::Expr>);

                    impl eval::Convert for Columns<'_> {
                        fn convert_expr_column(
                            &mut self,
                            stmt: &stmt::ExprColumn,
                        ) -> Option<stmt::Expr> {
                            let index = self
                                .0
                                .iter()
                                .position(|expr| match expr {
                                    stmt::Expr::Column(expr) => expr == stmt,
                                    _ => false,
                                })
                                .unwrap();

                            Some(stmt::Expr::project(stmt::Expr::arg(0), [index]))
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

                    eval::Func::try_convert_from_stmt(expr, project.args.clone(), convert).unwrap()
                });

                self.push_action(plan::QueryPk {
                    output: plan::Output {
                        var: output,
                        project,
                    },
                    table: table.id,
                    columns,
                    pk_filter: index_plan.index_filter,
                    filter: index_plan.result_filter,
                    post_filter,
                });

                output
            }
        } else {
            assert!(index_plan.post_filter.is_none());

            let get_by_key_input = self.plan_find_pk_by_index(&mut index_plan, input);
            let keys = eval::Func::identity(get_by_key_input.project.ret.clone());

            self.push_action(plan::GetByKey {
                input: Some(get_by_key_input),
                output: plan::Output {
                    var: output,
                    project,
                },
                table: table.id,
                keys,
                columns: self.schema.mapping_for(model).columns.clone(),
                post_filter: result_post_filter,
            });

            output
        }
    }

    fn plan_select_include(
        &mut self,
        base: ModelId,
        path: &stmt::Path,
        input: plan::VarId,
    ) -> Result<()> {
        // TODO: move this into verifier
        assert_eq!(base, path.root);

        let [step] = &path.projection[..] else {
            todo!()
        };

        let model = self.model(base);
        let field = &model.fields[*step];

        match &field.ty {
            FieldTy::HasMany(rel) => {
                let pair = rel.pair(&self.schema.app);

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

                let Some(out) =
                    self.plan_stmt_raw(&cx, stmt::Query::filter(rel.target, filter).into())?
                else {
                    todo!()
                };

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
                let Some(out) =
                    self.plan_stmt_raw(&cx, stmt::Query::filter(rel.target, filter).into())?
                else {
                    todo!()
                };

                // Associate target records with the source
                self.push_action(plan::Associate {
                    source: input,
                    target: out,
                    field: field.id,
                });
            }
            FieldTy::HasOne(rel) => {
                let pair = rel.pair(&self.schema.app);

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

                let Some(out) =
                    self.plan_stmt_raw(&cx, stmt::Query::filter(rel.target, filter).into())?
                else {
                    todo!()
                };

                // Associate target records with the source
                self.push_action(plan::Associate {
                    source: input,
                    target: out,
                    field: field.id,
                });
            }
            _ => todo!("field.ty={:#?}", field.ty),
        }

        Ok(())
    }

    fn rewrite_offset_after_as_filter(&self, stmt: &mut stmt::Query) {
        let Some(limit) = &mut stmt.limit else {
            return;
        };

        let Some(stmt::Offset::After(offset)) = limit.offset.take() else {
            return;
        };

        let Some(order_by) = &mut stmt.order_by else {
            return;
        };

        let stmt::ExprSet::Select(body) = &mut stmt.body else {
            todo!("stmt={stmt:#?}");
        };

        match offset {
            stmt::Expr::Value(stmt::Value::Record(_)) => {
                todo!()
            }
            stmt::Expr::Value(value) => {
                let expr =
                    self.rewrite_offset_after_field_as_filter(&order_by.exprs[0], value, true);
                if body.filter.is_true() {
                    body.filter = expr;
                } else {
                    body.filter = stmt::Expr::and(body.filter.take(), expr);
                }
            }
            _ => todo!(),
        }
    }

    fn rewrite_offset_after_field_as_filter(
        &self,
        order_by: &stmt::OrderByExpr,
        value: stmt::Value,
        last: bool,
    ) -> stmt::Expr {
        let op = match (order_by.order, last) {
            (Some(stmt::Direction::Desc), true) => stmt::BinaryOp::Lt,
            (Some(stmt::Direction::Desc), false) => stmt::BinaryOp::Le,
            (_, true) => stmt::BinaryOp::Gt,
            (_, false) => stmt::BinaryOp::Ge,
        };

        stmt::Expr::binary_op(order_by.expr.clone(), op, value)
    }
}
