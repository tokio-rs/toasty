use indexmap::IndexSet;
use toasty_core::stmt::{self, visit_mut};

use crate::engine::{
    eval,
    exec::{MergeQualification, NestedChild, NestedLevel},
    hir, mir,
    plan::HirPlanner,
    Engine, HirStatement,
};

#[derive(Debug)]
struct NestedMergePlanner<'a> {
    engine: &'a Engine,
    hir: &'a HirStatement,
    mir: &'a mut mir::Store,
    inputs: IndexSet<mir::NodeId>,
    /// Statements that must execute before the merge but whose output is not needed
    deps: IndexSet<mir::NodeId>,
    /// Statement stack, used to infer expression types
    stack: Vec<hir::StmtId>,
}

impl HirPlanner<'_> {
    /// Builds a nested merge operation for queries with sub-statement arguments
    /// in the returning clause.
    ///
    /// When a query has `Arg::Sub { returning: true, .. }` arguments
    /// (sub-statements used in the returning clause), those represent nested
    /// data that needs to be merged with their parent rows. This method
    /// constructs a `NestedMerge` execution plan that:
    ///
    /// 1. Identifies all batch-loaded inputs needed (parent and child queries)
    /// 2. Builds a tree structure mirroring the nesting hierarchy
    /// 3. For each level, captures:
    ///    - The source data (reference to batch-loaded results)
    ///    - How to filter child rows for each parent (qualification predicates)
    ///    - How to project the combined parent+children into the final shape
    ///
    /// The resulting `NestedMerge` will execute by:
    /// - Loading all batch data upfront - fetches all input data for all levels before processing
    /// - Processing each root row:
    ///   - For each nested child relationship, filters batch-loaded child data and recursively
    ///     merges matching rows with their own children
    ///   - Collects results into a list, or a single value if `single` is `true`
    ///   - Projects the final row with the current row and all nested children
    /// - Returning all merged rows with their nested data
    ///
    /// # Example
    ///
    /// For a query like:
    /// ```sql
    /// SELECT user.*, (SELECT * FROM todos WHERE user_id = user.id) as todos
    /// FROM users
    /// ```
    ///
    /// This builds a two-level merge where:
    /// - Root level: user rows from batch load
    /// - Nested level: todo rows filtered by user_id match, projected into a list
    ///
    /// Returns `None` if the statement has no sub-statements with `returning: true`.
    pub(super) fn plan_nested_merge(&mut self, stmt_id: hir::StmtId) -> Option<mir::NodeId> {
        let stmt_state = &self.hir[stmt_id];

        // Return if there is no nested merge to do
        let need_nested_merge = stmt_state.args.iter().any(|arg| {
            matches!(
                arg,
                hir::Arg::Sub {
                    returning: true,
                    ..
                }
            )
        });
        if !need_nested_merge {
            return None;
        }

        if stmt_state.stmt.as_ref().unwrap().is_insert() {
            // todo!("stmt_state={stmt_state:#?}");
            return None;
        }

        let nested_merge_planner = NestedMergePlanner {
            engine: self.engine,
            hir: self.hir,
            mir: &mut self.mir,
            inputs: IndexSet::new(),
            deps: IndexSet::new(),
            stack: vec![],
        };

        let node_id = nested_merge_planner.plan_nested_merge(stmt_id);
        Some(node_id)
    }
}

impl NestedMergePlanner<'_> {
    fn plan_nested_merge(mut self, root: hir::StmtId) -> mir::NodeId {
        self.stack.push(root);
        let root = self.plan_nested_level(root, 0);
        self.stack.pop();

        // let deps = self.deps;
        self.mir.insert_with_deps(
            mir::NestedMerge {
                inputs: self.inputs,
                root,
            },
            self.deps,
        )
    }

    fn plan_nested_child(&mut self, stmt_id: hir::StmtId, depth: usize) -> NestedChild {
        self.stack.push(stmt_id);

        let level = self.plan_nested_level(stmt_id, depth);
        let stmt_state = &self.hir[stmt_id];
        let selection = stmt_state.load_data_columns.get().unwrap();

        let ret = match stmt_state.stmt.as_deref().unwrap() {
            stmt::Statement::Query(query) => {
                let filter = self.build_filter_for_nested_child(stmt_id, selection, depth);

                NestedChild {
                    level,
                    qualification: MergeQualification::Predicate(filter),
                    single: query.single,
                }
            }
            stmt::Statement::Insert(insert) => NestedChild {
                level,
                qualification: MergeQualification::All,
                single: insert.source.single,
            },
            stmt => todo!("stmt={stmt:#?}"),
        };

        self.stack.pop();

        ret
    }

    fn plan_nested_level(&mut self, stmt_id: hir::StmtId, depth: usize) -> NestedLevel {
        let stmt_state = &self.hir[stmt_id];
        let stmt = stmt_state.stmt.as_deref().unwrap();
        let returning = stmt.returning_unwrap();

        let source;
        let mut nested = vec![];

        // Map the returning clause to projection expression
        let projection = match returning {
            stmt::Returning::Expr(expr) => {
                let (s, _) = self
                    .inputs
                    .insert_full(stmt_state.load_data_statement.get().unwrap());

                source = s;
                self.build_projection_from_expr(stmt_id, expr, depth, &mut nested)
            }
            _ => {
                let node_id = stmt_state.output.get().unwrap();

                let (s, _) = self.inputs.insert_full(node_id);
                source = s;

                // Flatten list (bit of a hack)
                let ty = match self.mir[node_id].ty().clone() {
                    stmt::Type::List(ty) => *ty,
                    ty => ty,
                };

                eval::Func::from_stmt(stmt::Expr::arg(0), vec![ty])
            }
        };

        NestedLevel {
            source,
            projection,
            nested,
        }
    }

    fn build_filter_arg_tys(&self) -> Vec<stmt::Type> {
        self.stack
            .iter()
            .map(|stmt_id| self.build_exec_statement_ty_for(*stmt_id))
            .collect()
    }

    fn build_projection_arg_tys(&self, nested_children: &[NestedChild]) -> Vec<stmt::Type> {
        let curr = self.stack.last().unwrap();
        let mut projection_arg_tys = vec![self.build_exec_statement_ty_for(*curr)];

        for nested in nested_children {
            projection_arg_tys.push(if nested.single {
                nested.level.projection.ret.clone()
            } else {
                stmt::Type::list(nested.level.projection.ret.clone())
            });
        }

        projection_arg_tys
    }

    fn build_exec_statement_ty_for(&self, stmt_id: hir::StmtId) -> stmt::Type {
        let stmt_state = &self.hir[stmt_id];
        let stmt = stmt_state.stmt.as_deref().unwrap();

        let cx = stmt::ExprContext::new_with_target(&*self.engine.schema, stmt);

        let mut fields = vec![];

        for expr_reference in stmt_state.load_data_columns.get().unwrap() {
            fields.push(cx.infer_expr_reference_ty(expr_reference));
        }

        stmt::Type::Record(fields)
    }

    fn build_projection_from_expr(
        &mut self,
        stmt_id: hir::StmtId,
        expr: &stmt::Expr,
        depth: usize,
        nested: &mut Vec<NestedChild>,
    ) -> eval::Func {
        let stmt_state = &self.hir[stmt_id];
        let selection = stmt_state.load_data_columns.get().unwrap();
        let mut projection = expr.clone();

        visit_mut::for_each_expr_mut(&mut projection, |expr| match expr {
            stmt::Expr::Arg(expr_arg) => match &stmt_state.args[expr_arg.position] {
                hir::Arg::Sub { stmt_id, .. } => {
                    let child_stmt_state = &self.hir[stmt_id];
                    let child_stmt = child_stmt_state.stmt.as_deref().unwrap();
                    let child_returning = child_stmt.returning_unwrap();

                    // If the child statement has a constant returning clause,
                    // then the nested merge can inline the returning directly
                    // instead of having to get the values from the expression.
                    match child_returning {
                        stmt::Returning::Value(returning_expr) if returning_expr.is_const() => {
                            match child_stmt {
                                stmt::Statement::Query(query) => {
                                    if query.single {
                                        let stmt::Expr::Value(v) = returning_expr else {
                                            todo!()
                                        };
                                        assert!(!v.is_list());
                                    }
                                }
                                stmt::Statement::Insert(insert) => {
                                    if insert.source.single {
                                        let stmt::Expr::Value(v) = returning_expr else {
                                            todo!()
                                        };
                                        assert!(!v.is_list());
                                    }
                                }
                                _ => {}
                            }

                            // For consistency, make sure the child statement's execution happens before this one.
                            self.deps
                                .insert(child_stmt_state.load_data_statement.get().unwrap());
                            *expr = returning_expr.clone();
                        }
                        _ => {
                            let nested_child = self.plan_nested_child(*stmt_id, depth + 1);
                            nested.push(nested_child);

                            // Taking the
                            *expr = stmt::Expr::arg(nested.len());
                        }
                    }
                }
                hir::Arg::Ref { .. } => todo!(),
            },
            stmt::Expr::Reference(expr_reference) => {
                let expr_column = expr_reference.as_expr_column_unwrap();
                debug_assert_eq!(0, expr_column.nesting);
                let index = selection.get_index_of(expr_reference).unwrap();
                *expr = stmt::Expr::arg_project(0, [index]);
            }
            _ => {}
        });

        let projection_arg_tys = self.build_projection_arg_tys(nested);
        eval::Func::from_stmt(projection, projection_arg_tys)
    }

    fn build_filter_for_nested_child(
        &self,
        stmt_id: hir::StmtId,
        selection: &IndexSet<stmt::ExprReference>,
        depth: usize,
    ) -> eval::Func {
        let stmt_state = &self.hir[stmt_id];
        let stmt::Statement::Query(query) = stmt_state.stmt.as_deref().unwrap() else {
            unreachable!()
        };
        let select = query.body.as_select_unwrap();

        // Extract the qualification. For now, we will just re-run the
        // entire where clause, but that can be improved later.
        let mut filter = select.filter.clone();

        visit_mut::for_each_expr_mut(&mut filter, |expr| match expr {
            stmt::Expr::Arg(expr_arg) => {
                let hir::Arg::Ref {
                    nesting,
                    stmt_id: target_id,
                    target_expr_ref,
                    ..
                } = &stmt_state.args[expr_arg.position]
                else {
                    todo!()
                };

                debug_assert!(*nesting > 0);

                // This is a bit of a roundabout way to get the data. We may
                // want to find a better way to track the info for more direct
                // access.
                let target_stmt = &self.hir[target_id];

                let target_exec_statement_index = target_stmt
                    .load_data_columns
                    .get()
                    .unwrap()
                    .get_index_of(target_expr_ref)
                    .unwrap();

                *expr = stmt::Expr::arg_project(depth - *nesting, [target_exec_statement_index]);
            }
            stmt::Expr::Reference(expr_reference) => {
                let index = selection.get_index_of(expr_reference).unwrap();
                *expr = stmt::Expr::arg_project(depth, [index]);
            }
            _ => {}
        });

        let filter_arg_tys = self.build_filter_arg_tys();
        eval::Func::from_stmt(filter.into_expr(), filter_arg_tys)
    }
}
