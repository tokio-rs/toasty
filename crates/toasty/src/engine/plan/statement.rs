use std::mem;

use indexmap::IndexSet;
use toasty_core::stmt::{self, visit_mut, Condition};

use crate::engine::{
    eval,
    hir::{self},
    index::{self, IndexPlan},
    mir,
    plan::HirPlanner,
};

#[derive(Debug)]
struct LoadData {
    /// MIR node inputs needed to load data associated with the statement
    inputs: IndexSet<mir::NodeId>,

    /// Columns to load
    columns: IndexSet<stmt::ExprReference>,

    /// When the statement data is batch loaded (single database query to load
    /// data for multiple statements), arguments are passed in in batches as
    /// well. For SQL, this is done using derived tables. This maps "input" to
    /// the derived table index.
    batch_load_args: IndexSet<usize>,
}

#[derive(Debug)]
struct ReturningInfo {
    /// The returning clause
    clause: Option<stmt::Returning>,

    /// MIR node inputs needed to evaluate the returning clause
    inputs: IndexSet<mir::NodeId>,

    /// Maps the returning clause input. It can either reference the current
    /// statement or a statement argument.
    args: Vec<ReturningArg>,

    /// Tracks the argument position representing `ReturningArg::Stmt`.
    arg_stmt: Option<usize>,
}

#[derive(Debug)]
enum ReturningArg {
    /// The result of the current statement's data loading phase
    Stmt,

    /// The result of a sub statement
    Input(usize),
}

struct PlanStatement<'a, 'b> {
    planner: &'a mut HirPlanner<'b>,
    stmt_id: hir::StmtId,
    stmt_info: &'b hir::StatementInfo,

    /// Planning information related ot how to load data to satisfy the statement.
    load_data: LoadData,

    /// Tracks information related to the returning clause
    returning: ReturningInfo,

    /// True if the statement's dependencies have been tracked
    did_take_deps: bool,
}

impl HirPlanner<'_> {
    pub(super) fn plan_statement(&mut self, stmt_id: hir::StmtId) {
        let stmt_info = &self.hir[stmt_id];

        // Check if the statement has already been planned
        if stmt_info.exec_statement.get().is_some() {
            return;
        }

        // First, plan dependency statements. These are statments that must run
        // before the current one but do not reference the current statement.
        for &dep_stmt_id in &stmt_info.deps {
            self.plan_statement(dep_stmt_id);
        }

        let mut stmt = stmt_info.stmt.as_deref().unwrap().clone();
        let returning = stmt.take_returning();

        // Delegate to PlanStatement
        let mut planner = PlanStatement {
            planner: self,
            stmt_id,
            stmt_info,
            load_data: LoadData {
                inputs: IndexSet::new(),
                columns: IndexSet::new(),
                batch_load_args: IndexSet::new(),
            },
            returning: ReturningInfo {
                clause: returning,
                inputs: IndexSet::new(),
                args: vec![],
                arg_stmt: None,
            },
            did_take_deps: false,
        };
        planner.plan(stmt);
    }
}

impl<'a, 'b> PlanStatement<'a, 'b> {
    // ===== Entry point =====

    fn plan(&mut self, mut stmt: stmt::Statement) {
        // Tracks if the original query is a single query.
        let single = stmt.as_query().map(|query| query.single).unwrap_or(false);
        if let Some(query) = stmt.as_query_mut() {
            query.single = false;
        }

        // Track sub-statements referenced in the returning clause as inputs, so their
        // results are available when building the return value.
        self.extract_inputs_from_returning(&mut stmt);

        // Visit the main statement's returning clause to extract needed columns
        self.extract_columns_from_returning(&mut stmt);

        // Process any args (sub statements or refs to parent statements) in the query's filter.
        self.extract_data_load_args(&mut stmt);

        // For each back ref, include the needed columns
        self.collect_back_ref_columns();

        // If there are any ref args, then the statement might need to be
        // rewritten to batch load all records for a NestedMerge operation.
        if !self.load_data.batch_load_args.is_empty() {
            self.rewrite_stmt_for_batch_load(&mut stmt);
        }

        let exec_stmt_node_id = self.plan_data_loading(stmt);

        // Track the exec statement operation node.
        self.stmt_info.exec_statement.set(Some(exec_stmt_node_id));

        // Now, for each back ref, we need to project the expression to what the
        // next statement expects.
        self.process_back_ref_projections(exec_stmt_node_id);

        // Track the selection for later use.
        // TODO: Do we actually need to track this on the statement?
        self.stmt_info
            .load_data_columns
            .set(mem::take(&mut self.load_data.columns))
            .unwrap();

        // Plan each child
        self.plan_child_statements();

        // Plans a NestedMerge if one is needed
        let output_node_id = self.plan_output_node(exec_stmt_node_id, single);

        self.stmt_info.output.set(Some(output_node_id));
    }

    // ===== Setup helpers =====

    fn extract_inputs_from_returning(&mut self, stmt: &mut stmt::Statement) {
        visit_mut::for_each_expr_mut(&mut self.returning.clause, |expr| {
            if let stmt::Expr::Arg(expr_arg) = expr {
                match &self.stmt_info.args[expr_arg.position] {
                    hir::Arg::Ref {
                        stmt_id: target_id, ..
                    } => {
                        // Find the node providing the data for the ref
                        let node_id = self.planner.hir[target_id].back_refs[&self.stmt_id]
                            .node_id
                            .get()
                            .unwrap();

                        let (index, _) = self.returning.inputs.insert_full(node_id);
                        assert_eq!(index, 0, "TODO");
                        // input.set(Some(index));
                        todo!("index={index:#?}");
                    }
                    hir::Arg::Sub {
                        stmt_id: target_id,
                        input,
                        returning: true,
                    } => {
                        // If there are back-refs, the exec statement is preloading
                        // data for a NestedMerge. Sub-statements will be loaded
                        // during the NestedMerge.
                        if !self.stmt_info.back_refs.is_empty() {
                            return;
                        }

                        let target_stmt_info = &self.planner.hir[target_id];

                        let node_id = if target_stmt_info.independent {
                            target_stmt_info.output.get().expect("bug")
                        } else {
                            // self.planner.hir[stmt_id].exec_statement.get().expect("bug")
                            todo!(
                                "what is going on? arg={:#?}; stmt={:#?}",
                                self.stmt_info.args[expr_arg.position],
                                target_stmt_info,
                            );
                        };

                        let (index, _) = self.returning.inputs.insert_full(node_id);
                        input.set(Some(index));
                    }
                    _ => todo!(),
                }
            }
        });
    }

    fn extract_columns_from_returning(&mut self, stmt: &mut stmt::Statement) {
        if let stmt::Statement::Insert(insert) = stmt {
            if insert.source.body.is_values() {
                self.extract_columns_and_args_from_insert_returning();
                return;
            }
        }

        self.extract_columns_from_stmt_returning()
    }

    fn extract_columns_and_args_from_insert_returning(&mut self) {
        visit_mut::for_each_expr_mut(&mut self.returning.clause, |expr| match expr {
            stmt::Expr::Reference(expr_reference) => {
                let position = *self.returning.arg_stmt.get_or_insert_with(|| {
                    let position = self.returning.args.len();
                    self.returning.args.push(ReturningArg::Stmt);
                    position
                });

                assert!(
                    expr_reference.is_column(),
                    "TODO: expr_reference = {expr:#?}"
                );
                let (index, _) = self.load_data.columns.insert_full(*expr_reference);
                *expr = stmt::Expr::arg_project(position, [index]);
            }
            stmt::Expr::Arg(expr_arg) => {
                let position = self.returning.args.len();
                self.returning
                    .args
                    .push(ReturningArg::Input(expr_arg.position));
                *expr = stmt::Expr::arg(position);
            }
            _ => {}
        });
    }

    fn extract_columns_from_stmt_returning(&mut self) {
        visit_mut::for_each_expr_mut(&mut self.returning.clause, |expr| {
            if let stmt::Expr::Reference(expr_reference) = expr {
                let (index, _) = self.load_data.columns.insert_full(*expr_reference);
                *expr = stmt::Expr::arg_project(0, [index]);
            }
        });
    }

    /// Extract arguments needed to perform data loading
    fn extract_data_load_args(&mut self, stmt: &mut stmt::Statement) {
        let Some(filter) = stmt.filter() else {
            return;
        };
        stmt::visit::for_each_expr(filter, |expr| {
            if let stmt::Expr::Arg(expr_arg) = expr {
                match &self.stmt_info.args[expr_arg.position] {
                    hir::Arg::Sub {
                        stmt_id: target_id,
                        returning,
                        input,
                        ..
                    } => {
                        debug_assert!(!returning, "the argument was found in a filter");
                        // Sub-statement arguments in the filter should only happen with !sql
                        debug_assert!(!self.planner.engine.capability().sql);

                        let node_id = self.planner.hir[target_id].output.get().expect("bug");
                        let (index, _) = self.load_data.inputs.insert_full(node_id);
                        input.set(Some(index));
                    }
                    hir::Arg::Ref {
                        stmt_id: target_id,
                        input,
                        batch_load_table_ref_index,
                        ..
                    } => {
                        // refs can be duplicated in the same statement
                        if input.get().is_some() {
                            return;
                        }

                        let target_stmt_info = &self.planner.hir[target_id];
                        let back_ref = &target_stmt_info.back_refs[&self.stmt_id];

                        // TODO: should we just use the data_load node ID?
                        let node_id = back_ref.node_id.get().unwrap();

                        let (index, _) = self.load_data.inputs.insert_full(node_id);
                        input.set(Some(index));

                        // If the target statement is a query, then we are in a batch-load scenario.
                        if target_stmt_info.stmt().is_query() {
                            let (batch_load_index, _) =
                                self.load_data.batch_load_args.insert_full(index);
                            batch_load_table_ref_index.set(Some(batch_load_index));
                        } else {
                            todo!()
                        }
                    }
                }
            }
        });
    }

    fn collect_back_ref_columns(&mut self) {
        for back_ref in self.stmt_info.back_refs.values() {
            for expr in &back_ref.exprs {
                self.load_data.columns.insert(*expr);
            }
        }
    }

    fn rewrite_stmt_for_batch_load(&mut self, stmt: &mut stmt::Statement) {
        if let stmt::Statement::Insert(stmt) = stmt {
            if self.planner.engine.capability().sql {
                self.rewrite_stmt_insert_for_batch_load_sql(stmt);
            } else {
                todo!()
            }
        } else if self.planner.engine.capability().sql {
            self.rewrite_stmt_query_for_batch_load_sql(stmt);
        } else {
            self.rewrite_stmt_query_for_batch_load_nosql(stmt);
        }
    }

    fn rewrite_stmt_query_for_batch_load_sql(&mut self, stmt: &mut stmt::Statement) {
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
                        stmt_id: target_id,
                        target_expr_ref,
                        batch_load_table_ref_index,
                        ..
                    } = &self.stmt_info.args[expr_arg.position]
                    else {
                        todo!()
                    };

                    let back_ref = &self.planner.hir[target_id].back_refs[&self.stmt_id];
                    let column = back_ref.exprs.get_index_of(target_expr_ref).unwrap();

                    // Rewrite reference the new `FROM`.
                    *expr = stmt::Expr::column(stmt::ExprColumn {
                        nesting: 0,
                        table: batch_load_table_ref_index.get().unwrap(),
                        column,
                    });
                }
                _ => {}
            }
        });

        let tables: Vec<stmt::TableRef> = self
            .load_data
            .batch_load_args
            .iter()
            .map(|position| stmt::TableRef::Arg(stmt::ExprArg::new(*position)))
            .collect();

        assert!(tables.len() <= 1, "TODO: handle more complicated cases");

        let sub_query = stmt::Select {
            returning: stmt::Returning::Expr(stmt::Expr::record([1])),
            source: stmt::Source::Table(stmt::SourceTable {
                tables,
                from_item: stmt::TableWithJoins {
                    relation: stmt::TableFactor::Table(stmt::SourceTableId(0)),
                    joins: vec![],
                },
            }),
            filter,
        };

        stmt.filter_mut_unwrap().set(stmt::Expr::exists(sub_query));
    }

    fn rewrite_stmt_query_for_batch_load_nosql(&mut self, stmt: &mut stmt::Statement) {
        let mut filter = stmt.filter_expr_mut();
        visit_mut::for_each_expr_mut(&mut filter, |expr| match expr {
            stmt::Expr::Reference(stmt::ExprReference::Column(expr_column)) => {
                debug_assert_eq!(0, expr_column.nesting);
            }
            stmt::Expr::Arg(expr_arg) => {
                let hir::Arg::Ref {
                    stmt_id: target_id,
                    target_expr_ref,
                    ..
                } = &self.stmt_info.args[expr_arg.position]
                else {
                    todo!()
                };

                let back_ref = &self.planner.hir[target_id].back_refs[&self.stmt_id];
                let column = back_ref.exprs.get_index_of(target_expr_ref).unwrap();

                *expr = stmt::Expr::arg(column);
            }
            _ => {}
        });

        assert!(
            self.load_data.batch_load_args.len() == 1,
            "TODO: handle more complicated cases"
        );
        let input = self.load_data.batch_load_args[0];

        if let Some(filter) = filter {
            let expr = filter.take();
            *filter = stmt::Expr::any(stmt::Expr::map(stmt::Expr::arg(input), expr));
        }
    }

    fn rewrite_stmt_insert_for_batch_load_sql(&mut self, stmt: &mut stmt::Insert) {
        /*
        let hir::Arg::Ref { stmt_id, .. } = &self.stmt_info.args[ref_source.position] else {
            todo!()
        };

        let target_stmt = &self.planner.hir[stmt_id];
        let target_stmt = target_stmt.stmt.as_deref().unwrap();

        assert!(target_stmt.is_insert(), "TODO");

        // For now, an insert statement referencing a parent is only supported when the
        // targeted insert statement is also an insert with a single row being inserted.
        let values = target_stmt.insert_source_unwrap().body.as_values_unwrap();
        assert_eq!(1, values.rows.len(), "TODO");

        let stmt::ExprSet::Values(values) = &mut stmt.source.body else {
            todo!()
        };

        for row in &mut values.rows {
            visit_mut::for_each_expr_mut(row, |expr| {
                if let stmt::Expr::Arg(expr_arg) = expr {
                    let hir::Arg::Ref {
                        batch_load_index: index,
                        input,
                        ..
                    } = &self.stmt_info.args[expr_arg.position]
                    else {
                        todo!()
                    };

                    *expr = stmt::Expr::arg_project(*expr_arg, [input.get().unwrap(), *index])
                }
            })
        }
        */
        todo!()
    }

    // ===== Plan data loading phase =====

    fn plan_data_loading(&mut self, stmt: stmt::Statement) -> mir::NodeId {
        if let Some(node_id) = self.plan_const_or_empty_statement(&stmt) {
            debug_assert!(
                stmt.is_query() || stmt.assignments().map(|a| a.is_empty()).unwrap_or(false),
                "planned a mutable statement as const; stmt={:#?}",
                stmt
            );
            node_id
        } else if self.planner.engine.capability().sql || stmt.is_insert() {
            self.plan_data_loading_sql(stmt)
        } else {
            self.plan_data_loading_nosql(stmt)
        }
    }

    fn plan_const_or_empty_statement(&mut self, stmt: &stmt::Statement) -> Option<mir::NodeId> {
        if stmt.is_const() {
            let stmt::Value::List(rows) = stmt.eval_const().unwrap() else {
                todo!()
            };

            return Some(
                self.insert_const(
                    rows,
                    self.planner
                        .engine
                        .infer_record_list_ty(&stmt, &self.load_data.columns),
                ),
            );
        }

        if stmt.assignments().map(|a| a.is_empty()).unwrap_or(false) {
            if self.returning.clause.is_some() {
                return Some(self.insert_const(
                    vec![stmt::Value::empty_sparse_record()],
                    stmt::Type::list(stmt::Type::empty_sparse_record()),
                ));
            } else {
                return Some(self.insert_const(
                    Vec::<stmt::Value>::new(),
                    stmt::Type::list(stmt::Type::empty_sparse_record()),
                ));
            }
        }

        None
    }

    // ===== SQL execution =====

    fn plan_data_loading_sql(&mut self, mut stmt: stmt::Statement) -> mir::NodeId {
        let const_returning = self.extract_insert_returning_as_const(&stmt);

        if !self.load_data.columns.is_empty() {
            stmt.set_returning(
                stmt::Expr::record(
                    self.load_data
                        .columns
                        .iter()
                        .map(|expr_reference| stmt::Expr::from(*expr_reference)),
                )
                .into(),
            );
        }

        let input_args: Vec<_> = self
            .load_data
            .inputs
            .iter()
            .map(|input| self.planner.mir.ty(*input).clone())
            .collect();

        let ty = self.planner.engine.infer_ty(&stmt, &input_args[..]);

        let node = if stmt.condition().is_some() {
            if let stmt::Statement::Update(stmt) = stmt {
                assert!(stmt.returning.is_none(), "TODO: stmt={stmt:#?}");
                assert!(
                    self.returning.clause.is_none(),
                    "TODO: returning={:#?}",
                    self.returning
                );

                if self.planner.engine.capability().cte_with_update {
                    mir::Operation::ExecStatement(Box::new(
                        self.plan_conditional_sql_query_as_cte(stmt, ty),
                    ))
                } else {
                    mir::Operation::ReadModifyWrite(Box::new(
                        self.plan_conditional_sql_query_as_rmw(stmt, ty),
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
                inputs: mem::take(&mut self.load_data.inputs),
                stmt,
                ty,
                conditional_update_with_no_returning: false,
            }))
        };

        // With SQL capability, we can just punt the details of execution to
        // the database's query planner.
        debug_assert!(!self.did_take_deps);
        let mut exec_statement_node = self.insert_mir_with_deps(node);

        if let Some((const_value, const_ty)) = const_returning {
            exec_statement_node = self.planner.mir.insert_with_deps(
                mir::Const {
                    value: const_value,
                    ty: const_ty,
                },
                [exec_statement_node],
            );
        }

        exec_statement_node
    }

    fn extract_insert_returning_as_const(
        &mut self,
        stmt: &stmt::Statement,
    ) -> Option<(stmt::Value, stmt::Type)> {
        let stmt::Statement::Insert(insert) = stmt else {
            return None;
        };

        if self.load_data.columns.is_empty() {
            return None;
        }

        let target = insert.target.as_table_unwrap();
        let Some(values) = insert.source.body.as_values() else {
            return None;
        };

        let mut indices = vec![];

        for expr_ref in &self.load_data.columns {
            let expr_col = expr_ref.as_expr_column_unwrap();
            debug_assert!(expr_col.nesting == 0, "expr_column={expr_col:#?}");

            let Some(index) = target
                .columns
                .iter()
                .enumerate()
                .find(|(_, column_id)| column_id.index == expr_col.column)
                .map(|(index, _)| index)
            else {
                todo!("insert returning referencing parent statement");
                // return None;
            };

            indices.push(index);
        }

        // Now extract the values for each row
        let mut result = Vec::with_capacity(values.rows.len());

        for row in &values.rows {
            // Build a record with only the requested fields
            let mut fields = Vec::with_capacity(indices.len());

            for &index in &indices {
                // Try to evaluate the expression to a constant value
                let value = row.entry(index)?.eval_const().ok()?;
                fields.push(value);
            }

            result.push(stmt::Value::record_from_vec(fields));
        }

        let ty = self
            .planner
            .engine
            .infer_record_list_ty(stmt, &self.load_data.columns);

        Some((stmt::Value::List(result), ty))
    }

    fn plan_conditional_sql_query_as_cte(
        &mut self,
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
            inputs: mem::take(&mut self.load_data.inputs),
            stmt,
            ty,
            conditional_update_with_no_returning: true,
        }
    }

    fn plan_conditional_sql_query_as_rmw(
        &mut self,
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
            .locks(if self.planner.engine.capability().select_for_update {
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
            inputs: mem::take(&mut self.load_data.inputs),
            read,
            write: write.into(),
            ty,
        }
    }

    // ===== NoSQL execution =====

    fn plan_data_loading_nosql(&mut self, stmt: stmt::Statement) -> mir::NodeId {
        if stmt.is_insert() {
            debug_assert!(self.load_data.columns.is_empty());
        }

        // Without SQL capability, we have to plan the execution of the
        // statement based on available indices.
        let mut index_plan = self.planner.engine.plan_index_path(&stmt);
        let pk_keys = self.try_build_pk_keys(&stmt, &index_plan);

        let post_filter = self.prepare_post_filter(&stmt, &mut index_plan, pk_keys.is_some());

        // Type of the final record.
        let ty = if self.load_data.columns.is_empty() {
            stmt::Type::Unit
        } else {
            self.planner
                .engine
                .infer_record_list_ty(&stmt, &self.load_data.columns)
        };

        let node_id = if index_plan.index.primary_key {
            self.plan_primary_key_execution(stmt, &mut index_plan, pk_keys, &ty)
        } else {
            self.plan_secondary_index_execution(stmt, &mut index_plan, &ty)
        };

        self.apply_post_filter(node_id, post_filter, ty)
    }

    fn plan_primary_key_execution(
        &mut self,
        stmt: stmt::Statement,
        index_plan: &mut index::IndexPlan,
        pk_keys: Option<eval::Func>,
        ty: &stmt::Type,
    ) -> mir::NodeId {
        if let Some(keys) = pk_keys {
            let get_by_key_input = self.build_get_by_key_input(keys, self.index_key_ty(index_plan));

            self.build_key_operation(&stmt, index_plan, get_by_key_input, ty)
        } else {
            let input = if self.load_data.inputs.is_empty() {
                None
            } else if self.load_data.inputs.len() == 1 {
                Some(self.load_data.inputs[0])
            } else {
                todo!()
            };

            self.insert_mir_with_deps(mir::QueryPk {
                input,
                table: index_plan.table_id(),
                columns: self.load_data.columns.clone(),
                pk_filter: index_plan.index_filter.take(),
                row_filter: index_plan.result_filter.take(),
                ty: ty.clone(),
            })
        }
    }

    fn plan_secondary_index_execution(
        &mut self,
        stmt: stmt::Statement,
        index_plan: &mut index::IndexPlan,
        ty: &stmt::Type,
    ) -> mir::NodeId {
        let inputs = mem::take(&mut self.load_data.inputs);
        assert!(index_plan.post_filter.is_none(), "TODO");
        assert!(inputs.len() <= 1, "TODO: inputs={:#?}", inputs);

        let index_key_ty = self.index_key_ty(index_plan);

        let get_by_key_input = self.insert_mir_with_deps(mir::FindPkByIndex {
            inputs,
            table: index_plan.index.on,
            index: index_plan.index.id,
            filter: index_plan.index_filter.take(),
            ty: index_key_ty,
        });

        self.build_key_operation(&stmt, index_plan, get_by_key_input, ty)
    }

    fn try_build_pk_keys(
        &mut self,
        stmt: &stmt::Statement,
        index_plan: &index::IndexPlan,
    ) -> Option<eval::Func> {
        // If the query can be reduced to fetching rows using a set of
        // primary-key keys, then `pk_keys` will be set to `Some(<keys>)`.
        if !index_plan.index.primary_key {
            return None;
        }

        let pk_keys_project_args = self
            .load_data
            .inputs
            .iter()
            .map(|node_id| self.planner.mir[node_id].ty().clone())
            .collect();

        // If using the primary key to find rows, try to convert the
        // filter expression to a set of primary-key keys.
        let cx = self.planner.engine.expr_cx_for(stmt);
        self.planner.engine.try_build_key_filter(
            cx,
            index_plan.index,
            &index_plan.index_filter,
            pk_keys_project_args,
        )
    }

    fn prepare_post_filter(
        &mut self,
        stmt: &stmt::Statement,
        index_plan: &mut index::IndexPlan,
        has_pk_keys: bool,
    ) -> Option<stmt::Expr> {
        let mut post_filter = index_plan.post_filter.clone();

        // If fetching rows using GetByKey, some databases do not support
        // applying additional filters to the rows before returning results.
        // In this case, the result_filter needs to be applied in-memory.
        if stmt.is_query() && (has_pk_keys || !index_plan.index.primary_key) {
            if let Some(result_filter) = index_plan.result_filter.take() {
                post_filter = Some(match post_filter {
                    Some(post_filter) => stmt::Expr::and(result_filter, post_filter),
                    None => result_filter,
                });
            }
        }

        debug_assert!(
            post_filter.is_none() || stmt.is_query(),
            "stmt={:#?}; post_filter={post_filter:#?}",
            stmt
        );

        // Make sure we are including columns needed to apply the post filter
        if let Some(post_filter) = &mut post_filter {
            visit_mut::for_each_expr_mut(post_filter, |expr| match expr {
                stmt::Expr::Reference(expr_reference) => {
                    let (index, _) = self.load_data.columns.insert_full(*expr_reference);
                    *expr = stmt::Expr::arg_project(0, [index]);
                }
                stmt::Expr::Arg(_) => todo!("expr={expr:#?}"),
                _ => {}
            });
        }

        post_filter
    }

    fn apply_post_filter(
        &mut self,
        mut node_id: mir::NodeId,
        post_filter: Option<stmt::Expr>,
        ty: stmt::Type,
    ) -> mir::NodeId {
        // If there is a post filter, we need to apply a filter step on the returned rows.
        if let Some(post_filter) = post_filter {
            let item_ty = ty.unwrap_list_ref();
            node_id = self.planner.mir.insert(mir::Filter {
                input: node_id,
                filter: eval::Func::from_stmt(post_filter, vec![item_ty.clone()]),
                ty,
            });
        }

        node_id
    }

    fn build_get_by_key_input(
        &mut self,
        keys: eval::Func,
        index_key_ty: stmt::Type,
    ) -> mir::NodeId {
        if keys.is_const() {
            self.insert_const(keys.eval_const(), index_key_ty)
        } else if keys.is_identity() {
            debug_assert_eq!(1, self.load_data.inputs.len(), "TODO");
            self.load_data.inputs[0]
        } else {
            let ty = stmt::Type::list(keys.ret.clone());
            // Gotta project
            self.planner.mir.insert(mir::Project {
                input: self.load_data.inputs[0],
                projection: keys,
                ty,
            })
        }
    }

    fn build_key_operation(
        &mut self,
        stmt: &stmt::Statement,
        index_plan: &mut index::IndexPlan,
        get_by_key_input: mir::NodeId,
        ty: &stmt::Type,
    ) -> mir::NodeId {
        match stmt {
            stmt::Statement::Query(_) => {
                debug_assert!(ty.is_list());
                self.insert_mir_with_deps(mir::GetByKey {
                    input: get_by_key_input,
                    table: index_plan.table_id(),
                    columns: self.load_data.columns.clone(),
                    ty: ty.clone(),
                })
            }
            stmt::Statement::Delete(_) => {
                debug_assert!(
                    ty.is_unit(),
                    "stmt={stmt:#?}; returning={:#?}; ty={ty:#?}",
                    self.returning.clause
                );
                self.insert_mir_with_deps(mir::DeleteByKey {
                    input: get_by_key_input,
                    table: index_plan.table_id(),
                    filter: index_plan.result_filter.take(),
                    ty: stmt::Type::Unit,
                })
            }
            stmt::Statement::Update(update_stmt) => self.insert_mir_with_deps(mir::UpdateByKey {
                input: get_by_key_input,
                table: index_plan.table_id(),
                assignments: update_stmt.assignments.clone(),
                filter: index_plan.result_filter.take(),
                condition: update_stmt.condition.expr.clone(),
                ty: ty.clone(),
            }),
            _ => todo!("stmt={stmt:#?}"),
        }
    }

    // ===== Finalization helpers =====

    fn process_back_ref_projections(&mut self, exec_stmt_node_id: mir::NodeId) {
        for back_ref in self.stmt_info.back_refs.values() {
            let projection = stmt::Expr::record(back_ref.exprs.iter().map(|expr_reference| {
                let index = self.load_data.columns.get_index_of(expr_reference).unwrap();
                stmt::Expr::arg_project(0, [index])
            }));

            let arg_ty = match self.planner.mir[exec_stmt_node_id].ty() {
                // Lists are flattened
                stmt::Type::List(ty) => (**ty).clone(),
                ty => ty.clone(),
            };

            let projection = eval::Func::from_stmt(projection, vec![arg_ty]);
            let ty = stmt::Type::list(projection.ret.clone());

            let project_node_id = self.planner.mir.insert(mir::Project {
                input: exec_stmt_node_id,
                projection,
                ty,
            });
            back_ref.node_id.set(Some(project_node_id));
        }
    }

    fn plan_child_statements(&mut self) {
        for arg in &self.stmt_info.args {
            let hir::Arg::Sub { stmt_id, .. } = arg else {
                continue;
            };

            self.planner.plan_statement(*stmt_id);
        }
    }

    fn plan_output_node(&mut self, exec_stmt_node_id: mir::NodeId, single: bool) -> mir::NodeId {
        // First check for nested merge
        if let Some(node_id) = self.planner.plan_nested_merge(self.stmt_id) {
            return node_id;
        }

        // Then handle returning clause
        if let Some(returning) = self.returning.clause.take() {
            match returning {
                stmt::Returning::Value(expr) => {
                    // Value variant contains a constant expression that can be evaluated
                    if let Ok(value) = expr.eval_const() {
                        let ty = value.infer_ty();

                        self.planner
                            .mir
                            .insert_with_deps(mir::Const { value, ty }, [exec_stmt_node_id])
                    } else {
                        let mut arg_tys = vec![];
                        let mut inputs = IndexSet::new();

                        for arg in &self.returning.args {
                            match arg {
                                ReturningArg::Stmt => {
                                    arg_tys.push(self.planner.mir[exec_stmt_node_id].ty().clone());
                                    inputs.insert(exec_stmt_node_id);
                                }
                                ReturningArg::Input(position) => {
                                    let hir::Arg::Sub { stmt_id, .. } =
                                        &self.stmt_info.args[*position]
                                    else {
                                        todo!()
                                    };

                                    let sub_target = &self.planner.hir[stmt_id];
                                    let sub_output = sub_target.output.get().unwrap();

                                    arg_tys.push(self.planner.mir[sub_output].ty().clone());
                                    inputs.insert(sub_output);
                                }
                            }
                        }

                        let eval = eval::Func::from_stmt(expr, arg_tys);

                        let node_id = self.insert_mir_with_deps(mir::Eval { inputs, eval });

                        if !self.stmt().is_query() {
                            self.planner.mir[node_id].deps.insert(exec_stmt_node_id);
                        }

                        node_id
                    }
                }
                stmt::Returning::Expr(returning) => {
                    // todo!(
                    //     "returning={returning:#?}; returning_args={:#?}; stmt_info.args={:#?}",
                    //     self.returning.args,
                    //     self.stmt_info.args
                    // );
                    let arg_ty = match self.planner.mir[exec_stmt_node_id].ty() {
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
                    self.insert_mir_with_deps(node)
                }
                returning => panic!("unexpected `stmt::Returning` kind; returning={returning:#?}"),
            }
        } else {
            if let Some(dependencies) = self.take_dependencies() {
                self.planner.mir[exec_stmt_node_id]
                    .deps
                    .extend(dependencies);
            }

            exec_stmt_node_id
        }
    }

    fn returning_expr_arg_tys(&self) -> Vec<stmt::Type> {
        todo!()
    }

    // ===== MIR/utility helpers =====

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

        self.planner.mir.insert(mir::Const { value, ty })
    }

    fn insert_mir_with_deps(&mut self, node: impl Into<mir::Node>) -> mir::NodeId {
        if let Some(dependencies) = self.take_dependencies() {
            self.planner.mir.insert_with_deps(node, dependencies)
        } else {
            self.planner.mir.insert(node)
        }
    }

    fn take_dependencies(&mut self) -> Option<impl Iterator<Item = mir::NodeId> + 'a> {
        if !self.did_take_deps {
            self.did_take_deps = true;
            Some(self.stmt_info.dependent_operations(self.planner.hir))
        } else {
            None
        }
    }

    fn index_key_ty(&self, index_plan: &IndexPlan) -> stmt::Type {
        // Type of the index key. Value for single index keys, record for
        // composite.
        stmt::Type::list(self.planner.engine.index_key_record_ty(index_plan.index))
    }

    fn stmt(&self) -> &stmt::Statement {
        self.stmt_info.stmt.as_deref().unwrap()
    }
}
