use indexmap::IndexSet;
use toasty_core::stmt::{self, visit_mut};

use crate::engine::{
    eval,
    exec::{MergeQualification, NestedChild, NestedLevel},
    hir, mir, Engine,
};

#[derive(Debug)]
struct NestedMergePlanner<'a> {
    engine: &'a Engine,
    store: &'a hir::Store,
    inputs: IndexSet<mir::NodeId>,
    /// Statement stack, used to infer expression types
    stack: Vec<hir::StmtId>,
}

impl super::PlanStatement<'_> {
    pub(super) fn plan_nested_merge(&mut self, stmt_id: hir::StmtId) -> Option<mir::NodeId> {
        let stmt_state = &self.store[stmt_id];

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

        let nested_merge_planner = NestedMergePlanner {
            engine: self.engine,
            store: self.store,
            inputs: IndexSet::new(),
            stack: vec![],
        };

        let nested_merge_materialization = nested_merge_planner.plan_nested_merge(stmt_id);
        let node_id = self.graph.insert(nested_merge_materialization);

        Some(node_id)
    }
}

impl NestedMergePlanner<'_> {
    fn plan_nested_merge(mut self, root: hir::StmtId) -> mir::NestedMerge {
        self.stack.push(root);
        let root = self.plan_nested_level(root, 0);
        self.stack.pop();

        mir::NestedMerge {
            inputs: self.inputs,
            root,
        }
    }

    fn plan_nested_child(&mut self, stmt_id: hir::StmtId, depth: usize) -> NestedChild {
        self.stack.push(stmt_id);

        let level = self.plan_nested_level(stmt_id, depth);
        let stmt_state = &self.store[stmt_id];
        let selection = stmt_state.exec_statement_selection.get().unwrap();

        let query = stmt_state.stmt.as_deref().unwrap().as_query().unwrap();
        let select = query.body.as_select_unwrap();

        // Extract the qualification. For now, we will just re-run the
        // entire where clause, but that can be improved later.
        let mut filter = select.filter.clone();

        visit_mut::for_each_expr_mut(&mut filter, |expr| match expr {
            stmt::Expr::Arg(expr_arg) => {
                let hir::Arg::Ref {
                    nesting,
                    stmt_id: target_id,
                    batch_load_index,
                    ..
                } = &stmt_state.args[expr_arg.position]
                else {
                    todo!()
                };

                debug_assert!(*nesting > 0);

                // This is a bit of a roundabout way to get the data. We may
                // want to find a better way to track the info for more direct
                // access.
                let target_stmt = &self.store[target_id];
                // The ExprReference based on the target's "self"
                let target_expr_reference =
                    &target_stmt.back_refs[&stmt_id].exprs[*batch_load_index];

                let target_exec_statement_index = target_stmt
                    .exec_statement_selection
                    .get()
                    .unwrap()
                    .get_index_of(target_expr_reference)
                    .unwrap();

                let _ = self.store[target_id]
                    .exec_statement_selection
                    .get()
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
        let filter = eval::Func::from_stmt(filter.into_expr(), filter_arg_tys);

        let ret = NestedChild {
            level,
            qualification: MergeQualification::Predicate(filter),
            single: query.single,
        };

        self.stack.pop();

        ret
    }

    fn plan_nested_level(&mut self, stmt_id: hir::StmtId, depth: usize) -> NestedLevel {
        let stmt_state = &self.store[stmt_id];
        let selection = stmt_state.exec_statement_selection.get().unwrap();

        // First, track the batch-load as a required input for the nested merge
        let (source, _) = self
            .inputs
            .insert_full(stmt_state.exec_statement.get().unwrap());

        let select = stmt_state.stmt.as_deref().unwrap().as_select_unwrap();

        let mut nested = vec![];

        // Map the returning clause to projection expression
        let mut projection = select.returning.as_expr_unwrap().clone();

        visit_mut::for_each_expr_mut(&mut projection, |expr| match expr {
            stmt::Expr::Arg(expr_arg) => match &stmt_state.args[expr_arg.position] {
                hir::Arg::Sub { stmt_id, .. } => {
                    let nested_child = self.plan_nested_child(*stmt_id, depth + 1);
                    nested.push(nested_child);

                    // Taking the
                    *expr = stmt::Expr::arg(nested.len());
                }
                hir::Arg::Ref { .. } => todo!(),
            },
            stmt::Expr::Reference(expr_reference) => {
                debug_assert_eq!(0, expr_reference.nesting());
                let index = selection.get_index_of(expr_reference).unwrap();
                *expr = stmt::Expr::arg_project(0, [index]);
            }
            _ => {}
        });

        let projection_arg_tys = self.build_projection_arg_tys(&nested);
        let projection = eval::Func::from_stmt(projection, projection_arg_tys);

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
        let mut ret = vec![self.build_exec_statement_ty_for(*curr)];

        for nested in nested_children {
            ret.push(if nested.single {
                nested.level.projection.ret.clone()
            } else {
                stmt::Type::list(nested.level.projection.ret.clone())
            });
        }

        ret
    }

    fn build_exec_statement_ty_for(&self, stmt_id: hir::StmtId) -> stmt::Type {
        let stmt_state = &self.store[stmt_id];
        let cx = stmt::ExprContext::new_with_target(
            &*self.engine.schema,
            stmt_state.stmt.as_deref().unwrap(),
        );

        let mut fields = vec![];

        for expr_reference in stmt_state.exec_statement_selection.get().unwrap() {
            fields.push(cx.infer_expr_reference_ty(expr_reference));
        }

        stmt::Type::Record(fields)
    }
}
