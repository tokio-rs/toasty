mod plan_nested_merge;

use index_vec::IndexVec;
use indexmap::IndexSet;
use toasty_core::stmt::{self, visit, visit_mut, Condition};

use crate::engine::{eval, hir, mir, planner::Planner, Engine};

#[derive(Debug)]
struct PlanStatement<'a> {
    engine: &'a Engine,

    /// Root statement and all nested statements.
    store: &'a hir::Store,

    /// Graph of operations needed to materialize the statement, in-progress
    graph: &'a mut mir::Store,
}

impl Planner<'_> {
    pub(super) fn plan_statement(&mut self) {
        PlanStatement {
            engine: self.engine,
            store: &self.store,
            graph: &mut self.graph,
        }
        .plan_materialize();
    }
}

impl PlanStatement<'_> {
    fn plan_materialize(&mut self) {
        let root_id = self.store.root_id();
        self.plan_materialize_statement(root_id);

        let exit = self.store.root().output.get().unwrap();
        self.compute_materialization_execution_order(exit);
    }

    fn plan_materialize_statement(&mut self, stmt_id: hir::StmtId) {
        let stmt_info = &self.store[stmt_id];
        let mut stmt = stmt_info.stmt.as_deref().unwrap().clone();

        // Check if the statement has already been planned
        if stmt_info.exec_statement.get().is_some() {
            return;
        }

        // First, plan dependency statements. These are statments that must run
        // before the current one but do not reference the current statement.
        for &dep_stmt_id in &stmt_info.deps {
            self.plan_materialize_statement(dep_stmt_id);
        }

        // Tracks if the original query is a single query.
        let single = stmt.as_query().map(|query| query.single).unwrap_or(false);
        if let Some(query) = stmt.as_query_mut() {
            query.single = false;
        }

        let mut returning = stmt.take_returning();

        // Columns to select
        let mut columns = IndexSet::new();

        // Materialization nodes this one depends on and uses the output of.
        let mut inputs = IndexSet::new();

        // Visit the main statement's returning clause to extract needed columns
        visit_mut::for_each_expr_mut(&mut returning, |expr| {
            match expr {
                stmt::Expr::Reference(expr_reference) => {
                    let (index, _) = columns.insert_full(*expr_reference);
                    *expr = stmt::Expr::arg_project(0, [index]);
                }
                stmt::Expr::Arg(expr_arg) => match &stmt_info.args[expr_arg.position] {
                    hir::Arg::Ref { .. } => {
                        todo!("refs in returning is not yet supported");
                    }
                    hir::Arg::Sub {
                        stmt_id,
                        input,
                        returning: true,
                    } => {
                        // If there are back-refs, the exec statement is preloading
                        // data for a NestedMerge. Sub-statements will be loaded
                        // during the NestedMerge.
                        if !stmt_info.back_refs.is_empty() {
                            return;
                        }

                        let node_id = self.store[stmt_id].exec_statement.get().expect("bug");

                        let (index, _) = inputs.insert_full(node_id);
                        input.set(Some(index));
                    }
                    _ => todo!(),
                },
                _ => {}
            }
        });

        // Track sub-statement arguments from filter
        visit_mut::for_each_expr_mut(&mut stmt.filter_mut(), |expr| {
            if let stmt::Expr::Arg(expr_arg) = expr {
                if let hir::Arg::Sub {
                    stmt_id: arg_stmt_id,
                    returning: false,
                    input,
                } = &stmt_info.args[expr_arg.position]
                {
                    debug_assert!(!self.engine.capability().sql);
                    debug_assert!(input.get().is_none());
                    let node_id = self.store[arg_stmt_id].output.get().expect("bug");

                    let (index, _) = inputs.insert_full(node_id);
                    input.set(Some(index));
                }
            }
        });

        // For each back ref, include the needed columns
        for back_ref in stmt_info.back_refs.values() {
            for expr in &back_ref.exprs {
                columns.insert(*expr);
            }
        }

        // If there are any ref args, then the statement needs to be rewritten
        // to batch load all records for a NestedMerge operation .
        let mut ref_source = None;

        for arg in &stmt_info.args {
            let hir::Arg::Ref {
                stmt_id: target_id,
                input,
                ..
            } = arg
            else {
                continue;
            };

            assert!(ref_source.is_none(), "TODO: handle more complex ref cases");
            assert!(
                !stmt.filter_or_default().is_false(),
                "TODO: handle const false filters"
            );

            // Find the back-ref for this arg
            let node_id = self.store[target_id].back_refs[&stmt_id]
                .node_id
                .get()
                .unwrap();

            let (index, _) = inputs.insert_full(node_id);
            ref_source = Some(stmt::ExprArg::new(index));
            input.set(Some(0));
        }

        if let Some(ref_source) = ref_source {
            if self.engine.capability().sql {
                // If targeting SQL, leverage the SQL query engine to handle most of the rewrite details.
                let mut filter = stmt
                    .filter_mut()
                    .map(|filter| filter.take())
                    .unwrap_or_default();

                visit_mut::for_each_expr_mut(&mut filter, |expr| {
                    match expr {
                        stmt::Expr::Reference(stmt::ExprReference::Column(expr_column)) => {
                            debug_assert_eq!(0, expr_column.nesting);
                            // We need to up the nesting to reflect that the filter is moved
                            // one level deeper.
                            expr_column.nesting += 1;
                        }
                        stmt::Expr::Arg(expr_arg) => {
                            let hir::Arg::Ref {
                                input,
                                batch_load_index: index,
                                ..
                            } = &stmt_info.args[expr_arg.position]
                            else {
                                todo!()
                            };

                            // Rewrite reference the new `FROM`.
                            *expr = stmt::Expr::column(stmt::ExprColumn {
                                nesting: 0,
                                table: input.get().unwrap(),
                                column: *index,
                            });
                        }
                        _ => {}
                    }
                });

                let sub_query = stmt::Select {
                    returning: stmt::Returning::Expr(stmt::Expr::record([1])),
                    source: stmt::Source::from(ref_source),
                    filter,
                };

                stmt.filter_mut_unwrap().set(stmt::Expr::exists(sub_query));
            } else {
                let mut filter = stmt.filter_expr_mut();
                visit_mut::for_each_expr_mut(&mut filter, |expr| match expr {
                    stmt::Expr::Reference(stmt::ExprReference::Column(expr_column)) => {
                        debug_assert_eq!(0, expr_column.nesting);
                    }
                    stmt::Expr::Arg(expr_arg) => {
                        let hir::Arg::Ref {
                            batch_load_index: index,
                            ..
                        } = &stmt_info.args[expr_arg.position]
                        else {
                            todo!()
                        };

                        *expr = stmt::Expr::arg(*index);
                    }
                    _ => {}
                });

                if let Some(filter) = filter {
                    let expr = filter.take();
                    *filter = stmt::Expr::any(stmt::Expr::map(ref_source, expr));
                }
            }
        }

        let mut dependencies = Some(stmt_info.dependent_materializations(self.store));

        let exec_stmt_node_id = if stmt.is_const() {
            debug_assert!(stmt_info.deps.is_empty());

            let stmt::Value::List(rows) = stmt.eval_const().unwrap() else {
                todo!()
            };

            // Don't bother querying and just return false
            self.insert_const(rows, self.engine.infer_record_list_ty(&stmt, &columns))

        // If the statement is an update statement without any assignments, then
        // it can be substituted with a constant.
        } else if stmt.assignments().map(|a| a.is_empty()).unwrap_or(false) {
            if returning.is_some() {
                self.insert_const(
                    vec![stmt::Value::empty_sparse_record()],
                    stmt::Type::list(stmt::Type::empty_sparse_record()),
                )
            } else {
                self.insert_const(vec![], stmt::Type::list(stmt::Type::empty_sparse_record()))
            }
        } else if self.engine.capability().sql || stmt.is_insert() {
            if !columns.is_empty() {
                stmt.set_returning(
                    stmt::Expr::record(
                        columns
                            .iter()
                            .map(|expr_reference| stmt::Expr::from(*expr_reference)),
                    )
                    .into(),
                );
            }

            let input_args: Vec<_> = inputs
                .iter()
                .map(|input| self.graph.ty(*input).clone())
                .collect();

            let ty = self.engine.infer_ty(&stmt, &input_args[..]);

            let node = if stmt.condition().is_some() {
                if let stmt::Statement::Update(stmt) = stmt {
                    assert!(stmt.returning.is_none(), "TODO: stmt={stmt:#?}");
                    assert!(returning.is_none(), "TODO: returning={returning:#?}");

                    if self.engine.capability().cte_with_update {
                        mir::Operation::ExecStatement(Box::new(
                            self.plan_materialize_conditional_sql_query_as_cte(inputs, stmt, ty),
                        ))
                    } else {
                        mir::Operation::ReadModifyWrite(Box::new(
                            self.plan_materialize_conditional_sql_query_as_rmw(inputs, stmt, ty),
                        ))
                    }
                } else {
                    todo!("stmt={stmt:#?}");
                }
            } else {
                debug_assert!(
                    stmt.returning()
                        .and_then(|returning| returning.as_expr())
                        .map(|expr| expr.is_record())
                        .unwrap_or(true),
                    "stmt={stmt:#?}"
                );
                // With SQL capability, we can just punt the details of execution to
                // the database's query planner.
                mir::Operation::ExecStatement(Box::new(mir::ExecStatement {
                    inputs,
                    stmt,
                    ty,
                    conditional_update_with_no_returning: false,
                }))
            };

            // With SQL capability, we can just punt the details of execution to
            // the database's query planner.
            self.graph
                .insert_with_deps(node, dependencies.take().unwrap())
        } else {
            // Without SQL capability, we have to plan the materialization of
            // the statement based on available indices.
            let mut index_plan = self.engine.plan_index_path(&stmt);
            let table_id = self.engine.resolve_table_for(&stmt).id;

            // If the query can be reduced to fetching rows using a set of
            // primary-key keys, then `pk_keys` will be set to `Some(<keys>)`.
            let mut pk_keys = None;

            // The post-filter is an expression that filters out returned rows
            // in-memory. To process this filter, Toasty needs to make sure that
            // any column referenced in the filter is included when fetching
            // data.
            let mut post_filter = index_plan.post_filter.clone();

            if index_plan.index.primary_key {
                let pk_keys_project_args = if ref_source.is_some() {
                    assert_eq!(inputs.len(), 1, "TODO");
                    let ty = self.graph[inputs[0]].ty();
                    vec![ty.unwrap_list_ref().clone()]
                } else {
                    inputs
                        .iter()
                        .map(|node_id| self.graph[node_id].ty().clone())
                        .collect()
                };

                // If using the primary key to find rows, try to convert the
                // filter expression to a set of primary-key keys.
                let cx = self.engine.expr_cx_for(&stmt);
                pk_keys = self.engine.try_build_key_filter(
                    cx,
                    index_plan.index,
                    &index_plan.index_filter,
                    pk_keys_project_args,
                );
            };

            // If fetching rows using GetByKey, some databases do not support
            // applying additional filters to the rows before returning results.
            // In this case, the result_filter needs to be applied in-memory.
            if stmt.is_query() && (pk_keys.is_some() || !index_plan.index.primary_key) {
                if let Some(result_filter) = index_plan.result_filter.take() {
                    post_filter = Some(match post_filter {
                        Some(post_filter) => stmt::Expr::and(result_filter, post_filter),
                        None => result_filter,
                    });
                }
            }

            debug_assert!(
                post_filter.is_none() || stmt.is_query(),
                "stmt={stmt:#?}; post_filter={post_filter:#?}"
            );

            // Make sure we are including columns needed to apply the post filter
            if let Some(post_filter) = &mut post_filter {
                visit_mut::for_each_expr_mut(post_filter, |expr| match expr {
                    stmt::Expr::Reference(expr_reference) => {
                        let (index, _) = columns.insert_full(*expr_reference);
                        *expr = stmt::Expr::arg_project(0, [index]);
                    }
                    stmt::Expr::Arg(_) => todo!("expr={expr:#?}"),
                    _ => {}
                });
            }

            // Type of the final record.
            let ty = if columns.is_empty() {
                stmt::Type::Unit
            } else {
                self.engine.infer_record_list_ty(&stmt, &columns)
            };

            // Type of the index key. Value for single index keys, record for
            // composite.
            let index_key_ty = stmt::Type::list(self.engine.index_key_record_ty(index_plan.index));

            let mut node_id = if index_plan.index.primary_key {
                if let Some(keys) = pk_keys {
                    let get_by_key_input = if keys.is_const() {
                        self.insert_const(keys.eval_const(), index_key_ty)
                    } else if keys.is_identity() {
                        debug_assert_eq!(1, inputs.len(), "TODO");
                        inputs[0]
                    } else {
                        debug_assert!(ref_source.is_some(), "TODO");
                        let ty = stmt::Type::list(keys.ret.clone());
                        // Gotta project
                        self.graph.insert(mir::Project {
                            input: inputs[0],
                            projection: keys,
                            ty,
                        })
                    };

                    match stmt {
                        stmt::Statement::Query(_) => {
                            debug_assert!(ty.is_list());
                            self.graph.insert_with_deps(
                                mir::GetByKey {
                                    input: get_by_key_input,
                                    table: table_id,
                                    columns: columns.clone(),
                                    ty: ty.clone(),
                                },
                                dependencies.take().into_iter().flatten(),
                            )
                        }
                        stmt::Statement::Delete(_) => {
                            debug_assert!(
                                ty.is_unit(),
                                "stmt={stmt:#?}; returning={returning:#?}; ty={ty:#?}"
                            );
                            self.graph.insert_with_deps(
                                mir::DeleteByKey {
                                    input: get_by_key_input,
                                    table: table_id,
                                    filter: index_plan.result_filter,
                                    ty: stmt::Type::Unit,
                                },
                                dependencies.take().into_iter().flatten(),
                            )
                        }
                        stmt::Statement::Update(stmt) => self.graph.insert_with_deps(
                            mir::UpdateByKey {
                                input: get_by_key_input,
                                table: table_id,
                                assignments: stmt.assignments,
                                filter: index_plan.result_filter,
                                condition: stmt.condition.expr,
                                ty: ty.clone(),
                            },
                            dependencies.take().into_iter().flatten(),
                        ),
                        _ => todo!("stmt={stmt:#?}"),
                    }
                } else {
                    let input = if inputs.is_empty() {
                        None
                    } else if inputs.len() == 1 {
                        Some(inputs[0])
                    } else {
                        todo!()
                    };

                    self.graph.insert_with_deps(
                        mir::QueryPk {
                            input,
                            table: table_id,
                            columns: columns.clone(),
                            pk_filter: index_plan.index_filter,
                            row_filter: index_plan.result_filter,
                            ty: ty.clone(),
                        },
                        dependencies.take().into_iter().flatten(),
                    )
                }
            } else {
                assert!(index_plan.post_filter.is_none(), "TODO");
                assert!(inputs.len() <= 1, "TODO: inputs={inputs:#?}");

                // Args not supportd yet...
                visit::for_each_expr(&index_plan.index_filter, |expr| {
                    if let stmt::Expr::Arg(expr_arg) = expr {
                        debug_assert_eq!(0, expr_arg.position, "TODO; index_plan={index_plan:#?}");
                        debug_assert!(ref_source.is_none() || ref_source == Some(*expr_arg));
                    }
                });

                let get_by_key_input = self.graph.insert_with_deps(
                    mir::FindPkByIndex {
                        inputs,
                        table: index_plan.index.on,
                        index: index_plan.index.id,
                        filter: index_plan.index_filter.take(),
                        ty: index_key_ty,
                    },
                    dependencies.take().into_iter().flatten(),
                );

                match stmt {
                    stmt::Statement::Query(_) => {
                        debug_assert!(ty.is_list());
                        self.graph.insert_with_deps(
                            mir::GetByKey {
                                input: get_by_key_input,
                                table: table_id,
                                columns: columns.clone(),
                                ty: ty.clone(),
                            },
                            dependencies.take().into_iter().flatten(),
                        )
                    }
                    stmt::Statement::Delete(_) => {
                        debug_assert!(
                            ty.is_unit(),
                            "stmt={stmt:#?}; returning={returning:#?}; ty={ty:#?}"
                        );
                        self.graph.insert_with_deps(
                            mir::DeleteByKey {
                                input: get_by_key_input,
                                table: table_id,
                                filter: index_plan.result_filter,
                                ty: stmt::Type::Unit,
                            },
                            dependencies.take().into_iter().flatten(),
                        )
                    }
                    stmt::Statement::Update(stmt) => self.graph.insert_with_deps(
                        mir::UpdateByKey {
                            input: get_by_key_input,
                            table: table_id,
                            assignments: stmt.assignments,
                            filter: index_plan.result_filter,
                            condition: stmt.condition.expr,
                            ty: ty.clone(),
                        },
                        dependencies.take().into_iter().flatten(),
                    ),
                    _ => todo!("stmt={stmt:#?}"),
                }
            };

            // If there is a post filter, we need to apply a filter step on the returned rows.
            if let Some(post_filter) = post_filter {
                let item_ty = ty.unwrap_list_ref();
                node_id = self.graph.insert(mir::Filter {
                    input: node_id,
                    filter: eval::Func::from_stmt(post_filter, vec![item_ty.clone()]),
                    ty,
                });
            }

            node_id
        };

        // Track the exec statement materialization node.
        stmt_info.exec_statement.set(Some(exec_stmt_node_id));

        // Now, for each back ref, we need to project the expression to what the
        // next statement expects.
        for back_ref in stmt_info.back_refs.values() {
            let projection = stmt::Expr::record(back_ref.exprs.iter().map(|expr_reference| {
                let index = columns.get_index_of(expr_reference).unwrap();
                stmt::Expr::arg_project(0, [index])
            }));

            let arg_ty = self.graph[exec_stmt_node_id].ty().unwrap_list_ref().clone();
            let projection = eval::Func::from_stmt(projection, vec![arg_ty]);
            let ty = stmt::Type::list(projection.ret.clone());

            let project_node_id = self.graph.insert(mir::Project {
                input: exec_stmt_node_id,
                projection,
                ty,
            });
            back_ref.node_id.set(Some(project_node_id));
        }

        // Track the selection for later use.
        stmt_info.exec_statement_selection.set(columns).unwrap();

        // Plan each child
        for arg in &stmt_info.args {
            let hir::Arg::Sub { stmt_id, .. } = arg else {
                continue;
            };

            self.plan_materialize_statement(*stmt_id);
        }

        // Plans a NestedMerge if one is needed
        let output_node_id = if let Some(node_id) = self.plan_nested_merge(stmt_id) {
            node_id
        } else if let Some(returning) = returning {
            debug_assert!(
                !single || ref_source.is_some(),
                "TODO: single queries not supported here"
            );

            match returning {
                stmt::Returning::Value(returning) => {
                    let ty = returning.infer_ty();

                    let stmt::Value::List(rows) = returning else {
                        todo!(
                            "unexpected returning type; returning={returning:#?}; stmt={:#?}",
                            stmt_info.stmt
                        )
                    };

                    self.graph
                        .insert_with_deps(mir::Const { value: rows, ty }, [exec_stmt_node_id])
                }
                stmt::Returning::Expr(returning) => {
                    let arg_ty = match self.graph[exec_stmt_node_id].ty() {
                        stmt::Type::List(ty) => vec![(**ty).clone()],
                        stmt::Type::Unit => vec![],
                        _ => todo!(),
                    };

                    let projection = eval::Func::from_stmt(returning, arg_ty);
                    let ty = stmt::Type::list(projection.ret.clone());

                    let node = mir::Project {
                        input: exec_stmt_node_id,
                        projection,
                        ty,
                    };

                    // Plan the final projection to handle the returning clause.
                    if let Some(deps) = dependencies.take() {
                        self.graph.insert_with_deps(node, deps)
                    } else {
                        self.graph.insert(node)
                    }
                }
                returning => panic!("unexpected `stmt::Returning` kind; returning={returning:#?}"),
            }
        } else {
            if let Some(deps) = dependencies.take() {
                self.graph[exec_stmt_node_id].deps.extend(deps);
            }

            exec_stmt_node_id
        };

        debug_assert!(dependencies.is_none());

        stmt_info.output.set(Some(output_node_id));
    }

    // plan_materialize_conditional_sql_query_as_cte
    fn plan_materialize_conditional_sql_query_as_cte(
        &self,
        inputs: IndexSet<mir::NodeId>,
        stmt: stmt::Update,
        ty: stmt::Type,
    ) -> mir::ExecStatement {
        let Some(condition) = stmt.condition.expr else {
            panic!("conditional update without condition");
        };

        let Some(filter) = stmt.filter.expr else {
            panic!("conditional update without filter");
        };

        let stmt::UpdateTarget::Table(target) = stmt.target.clone() else {
            panic!("conditional update without table");
        };

        let mut ctes = vec![];

        // Select from update table without the update condition.
        ctes.push(stmt::Cte {
            query: stmt::Query::builder(target)
                .filter(filter.clone())
                .returning(vec![
                    stmt::Expr::count_star(),
                    stmt::FuncCount {
                        arg: None,
                        filter: Some(Box::new(condition)),
                    }
                    .into(),
                ])
                .build(),
        });

        let returning_len = match &stmt.returning {
            Some(stmt::Returning::Expr(expr)) => {
                let stmt::Expr::Record(expr_record) = expr else {
                    panic!("returning must be a record");
                };

                expr_record.fields.len()
            }
            Some(_) => todo!(),
            None => 0,
        };

        // The update statement. The update condition is expressed using the select above
        ctes.push(stmt::Cte {
            query: stmt::Query::new(stmt::Update {
                target: stmt.target,
                assignments: stmt.assignments,
                filter: stmt::Filter::new(stmt::Expr::and(
                    filter,
                    // SELECT found.count(*) = found.count(CONDITION) FROM found
                    stmt::Expr::stmt(stmt::Select {
                        source: stmt::TableRef::Cte {
                            nesting: 2,
                            index: 0,
                        }
                        .into(),
                        filter: true.into(),
                        returning: stmt::Returning::Expr(stmt::Expr::record_from_vec(vec![
                            stmt::Expr::eq(
                                stmt::ExprColumn {
                                    nesting: 0,
                                    table: 0,
                                    column: 0,
                                },
                                stmt::ExprColumn {
                                    nesting: 0,
                                    table: 0,
                                    column: 1,
                                },
                            ),
                        ])),
                    }),
                )),
                condition: Condition::default(),
                returning: Some(
                    stmt.returning
                        // TODO: hax
                        .unwrap_or_else(|| {
                            stmt::Returning::Expr(stmt::Expr::record_from_vec(vec![
                                stmt::Expr::from("hello"),
                            ]))
                        }),
                ),
            }),
        });

        let mut columns = vec![
            stmt::Expr::column(stmt::ExprColumn {
                nesting: 0,
                table: 0,
                column: 0,
            }),
            stmt::Expr::column(stmt::ExprColumn {
                nesting: 0,
                table: 0,
                column: 1,
            }),
        ];

        for i in 0..returning_len {
            columns.push(stmt::Expr::column(stmt::ExprColumn {
                nesting: 0,
                table: 1,
                column: i,
            }));
        }

        let stmt = stmt::Query::builder(stmt::Select {
            source: stmt::Source::table_with_joins(
                vec![
                    stmt::TableRef::Cte {
                        nesting: 0,
                        index: 0,
                    },
                    stmt::TableRef::Cte {
                        nesting: 0,
                        index: 1,
                    },
                ],
                stmt::TableWithJoins {
                    relation: stmt::TableFactor::Table(stmt::SourceTableId(0)),
                    joins: vec![stmt::Join {
                        table: stmt::SourceTableId(1),
                        constraint: stmt::JoinOp::Left(stmt::Expr::from(true)),
                    }],
                },
            ),
            filter: stmt::Filter::new(true),
            returning: stmt::Returning::Expr(stmt::Expr::record_from_vec(columns)),
        })
        .with(ctes)
        .build()
        .into();

        mir::ExecStatement {
            inputs,
            stmt,
            ty,
            conditional_update_with_no_returning: true,
        }
    }

    fn plan_materialize_conditional_sql_query_as_rmw(
        &mut self,
        inputs: IndexSet<mir::NodeId>,
        stmt: stmt::Update,
        ty: stmt::Type,
    ) -> mir::ReadModifyWrite {
        // For now, no returning supported
        assert!(stmt.returning.is_none(), "TODO: support returning");

        let Some(condition) = stmt.condition.expr else {
            panic!("conditional update without condition");
        };

        let Some(filter) = stmt.filter.expr else {
            panic!("conditional update without filter");
        };

        let stmt::UpdateTarget::Table(target) = stmt.target.clone() else {
            panic!("conditional update without table");
        };

        // Neither SQLite nor MySQL support CTE with update. We should transform
        // the conditional update into a transaction with checks between.

        let read = stmt::Query::builder(target)
            .filter(filter.clone())
            .returning(vec![
                stmt::Expr::count_star(),
                stmt::FuncCount {
                    arg: None,
                    filter: Some(Box::new(condition)),
                }
                .into(),
            ])
            .locks(if self.engine.capability().select_for_update {
                vec![stmt::Lock::Update]
            } else {
                vec![]
            })
            .build();

        let write = stmt::Update {
            target: stmt.target,
            assignments: stmt.assignments,
            filter: stmt::Filter::new(filter),
            condition: stmt::Condition::default(),
            returning: None,
        };

        mir::ReadModifyWrite {
            inputs,
            read,
            write: write.into(),
            ty,
        }
    }

    fn compute_materialization_execution_order(&mut self, exit: mir::NodeId) {
        debug_assert!(self.graph.execution_order.is_empty());
        compute_materialization_execution_order2(
            exit,
            &self.graph.store,
            &mut self.graph.execution_order,
        );
    }

    #[track_caller]
    fn insert_const(&mut self, value: impl Into<stmt::Value>, ty: stmt::Type) -> mir::NodeId {
        let value = value.into();

        // Type check
        debug_assert!(
            ty.is_list(),
            "const types must be of type `stmt::Type::List`"
        );
        debug_assert!(
            value.is_a(&ty),
            "const type mismatch; expected={ty:#?}; actual={value:#?}",
        );

        self.graph.insert(mir::Const {
            value: value.unwrap_list(),
            ty,
        })
    }
}

fn compute_materialization_execution_order2(
    node_id: mir::NodeId,
    graph: &IndexVec<mir::NodeId, mir::Node>,
    execution_order: &mut Vec<mir::NodeId>,
) {
    let node = &graph[node_id];

    if node.visited.get() {
        return;
    }

    node.visited.set(true);

    for &dep_id in &node.deps {
        let dep = &graph[dep_id];
        dep.num_uses.set(dep.num_uses.get() + 1);

        compute_materialization_execution_order2(dep_id, graph, execution_order);
    }

    execution_order.push(node_id);
}
