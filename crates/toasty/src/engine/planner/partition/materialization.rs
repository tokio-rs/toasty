use std::cell::{Cell, OnceCell};

use indexmap::IndexSet;
use toasty_core::{
    stmt::{self, visit_mut, ExprReference},
    Schema,
};

use crate::engine::{
    eval,
    plan::{self, NestedLevel, NestedMerge},
    planner::partition::{Arg, StatementState, StmtId},
};

#[derive(Debug)]
pub(crate) struct MaterializationGraph {
    /// Nodes in the graph
    pub(crate) nodes: Vec<MaterializationNode>,

    /// Order of execution
    pub(crate) execution_order: Vec<NodeId>,
}

#[derive(Debug)]
pub(crate) struct MaterializationNode {
    /// Materialization kind
    pub(crate) kind: MaterializationKind,

    /// Variable where the output is stored
    pub(crate) var: Cell<Option<plan::VarId>>,

    /// Used for topo-sort
    visited: Cell<bool>,
}

type NodeId = usize;

/// Materialization operation
#[derive(Debug)]
pub(crate) enum MaterializationKind {
    /// Execute a database query
    ExecStatement {
        /// Inputs needed to reify the statement
        inputs: IndexSet<NodeId>,

        /// The database query to execute
        stmt: stmt::Statement,
    },

    /// Execute a nested merge
    NestedMerge {
        inputs: IndexSet<NodeId>,

        /// The root nested merge level
        root: plan::NestedLevel,
    },

    /// Projection operation - transforms records
    Project {
        /// Input required to perform the projection
        input: NodeId,

        /// Projection expression
        projection: stmt::Expr,
    },
}

#[derive(Debug)]
struct PlanMaterialization<'a> {
    schema: &'a Schema,

    /// Root statement and all nested statements.
    stmts: &'a [StatementState],

    /// Graph of operations needed to materialize the statement, in-progress
    graph: MaterializationGraph,
}

#[derive(Debug)]
struct PlanNestedMerge<'a> {
    schema: &'a Schema,
    stmts: &'a [StatementState],
    graph: &'a mut MaterializationGraph,
    inputs: IndexSet<NodeId>,
    /// Statement stack, used to infer expression types
    stack: Vec<StmtId>,
}

impl super::Planner<'_> {
    pub(super) fn plan_materializations(
        &self,
        stmts: &[StatementState],
        root: StmtId,
    ) -> MaterializationGraph {
        let mut plan_materialization = PlanMaterialization {
            schema: self.schema,
            stmts,
            graph: MaterializationGraph::new(),
        };
        plan_materialization.build_graph(root);

        let exit = stmts[root.0].output.get().unwrap();

        plan_materialization.compute_execution_order(exit);
        plan_materialization.graph
    }
}

impl PlanMaterialization<'_> {
    fn build_graph(&mut self, stmt_id: StmtId) {
        self.plan_stmt_materialization(stmt_id);
    }

    fn plan_stmt_materialization(&mut self, stmt_id: StmtId) {
        let stmt_state = &self.stmts[stmt_id.0];
        let mut stmt = stmt_state.stmt.as_deref().unwrap().clone();

        // Get the returning clause
        let stmt::Statement::Query(query) = &mut stmt else {
            panic!()
        };
        let stmt::ExprSet::Select(select) = &mut query.body else {
            panic!()
        };
        let stmt::Returning::Expr(returning) = &mut select.returning else {
            panic!()
        };
        // Take the returning clause out. This will be modified later.
        let mut returning = returning.take();

        // Columns to select
        let mut columns = IndexSet::new();

        // Materialization nodes this one depends on and uses the output of.
        let mut inputs = IndexSet::new();

        // Visit the main statement's returning clause to extract needed columns
        visit_mut::for_each_expr_mut(&mut returning, |expr| {
            match expr {
                stmt::Expr::Reference(expr_reference) => {
                    let (index, _) = columns.insert_full(expr_reference.clone());
                    *expr = stmt::Expr::arg_project(0, [index]);
                }
                stmt::Expr::Arg(expr_arg) => match &stmt_state.args[expr_arg.position] {
                    Arg::Ref { .. } => {
                        // let (index, _) = inputs.insert_full(*stmt_id);
                        // *input = Some(index);
                        todo!("refs in returning is not yet supported");
                    }
                    Arg::Sub { stmt_id, input } => {
                        // If there are back-refs, the exec statement is preloading
                        // data for a NestedMerge. Sub-statements will be loaded
                        // during the NestedMerge.
                        if !stmt_state.back_refs.is_empty() {
                            return;
                        }

                        let node_id = self.stmts[stmt_id.0].exec_statement.get().expect("bug");

                        let (index, _) = inputs.insert_full(node_id);
                        input.set(Some(index));
                    }
                },
                _ => {}
            }
        });

        // For each back ref, include the needed columns
        for back_ref in stmt_state.back_refs.values() {
            for expr in &back_ref.exprs {
                columns.insert(expr.clone());
            }
        }

        // If there are any ref args, then the statement needs to be rewritten
        // to batch load all records for a NestedMerge operation .
        let mut ref_source = None;

        for arg in &self.stmts[stmt_id.0].args {
            let Arg::Ref {
                stmt_id: target_id,
                input,
                ..
            } = arg
            else {
                continue;
            };

            assert!(ref_source.is_none(), "TODO: handle more complex ref cases");

            // Find the back-ref for this arg
            let node_id = self.stmts[target_id.0].back_refs[&stmt_id]
                .node_id
                .get()
                .unwrap();
            let (index, _) = inputs.insert_full(node_id);
            ref_source = Some(stmt::ExprArg::new(index));
            input.set(Some(0));
        }

        // Rewrite the filter to batch load all possible records that
        // will be needed to materialize the original statement.
        if let Some(ref_source) = ref_source {
            /*
            -- Step 1: Store filtered users
            CREATE TEMP TABLE temp_users AS
            SELECT * FROM users WHERE users.active = true;

            -- Step 2: Fetch all potentially matching todos
            SELECT todos.*
            FROM todos
            WHERE EXISTS (
              SELECT 1 FROM temp_users u
              WHERE todos.user_id = u.id
              AND todos.created_at > u.created_at
              AND todos.priority > 3
            );
                 */

            visit_mut::for_each_expr_mut(&mut select.filter, |expr| {
                match expr {
                    stmt::Expr::Reference(stmt::ExprReference::Column { nesting, .. }) => {
                        debug_assert_eq!(0, *nesting);
                        // We need to up the nesting to reflect that the filter is moved
                        // one level deeper.
                        *nesting += 1;
                    }
                    stmt::Expr::Arg(expr_arg) => {
                        let Arg::Ref { input, index, .. } = &stmt_state.args[expr_arg.position]
                        else {
                            todo!()
                        };

                        // Rewrite reference the new `FROM`.
                        *expr = stmt::ExprReference::Column {
                            nesting: 0,
                            table: input.get().unwrap(),
                            column: *index,
                        }
                        .into();
                    }
                    _ => {}
                }
            });

            let sub_query = stmt::Select {
                returning: stmt::Returning::Expr(stmt::Expr::record([1])),
                source: stmt::Source::from(ref_source),
                filter: select.filter.take(),
            };

            select.filter = stmt::Expr::exists(sub_query);
        }

        select.returning = stmt::Returning::Expr(stmt::Expr::record(
            columns
                .iter()
                .map(|expr_reference| stmt::Expr::from(expr_reference.clone())),
        ));

        // Create the exec statement materialization node.
        let exec_stmt_node_id = self.graph.nodes.len();
        self.graph
            .nodes
            .push(MaterializationKind::ExecStatement { inputs, stmt }.into());

        // Track the exec statement materialization node.
        stmt_state.exec_statement.set(Some(exec_stmt_node_id));

        // Now, for each back ref, we need to project the expression to what the
        // next statement expects.
        for back_ref in stmt_state.back_refs.values() {
            let projection = stmt::Expr::record(back_ref.exprs.iter().map(|expr_reference| {
                let index = columns.get_index_of(expr_reference).unwrap();
                stmt::Expr::arg_project(0, [index])
            }));

            let project_node_id = self.graph.nodes.len();
            self.graph.nodes.push(
                MaterializationKind::Project {
                    input: exec_stmt_node_id,
                    projection,
                }
                .into(),
            );
            back_ref.node_id.set(Some(project_node_id));
        }

        // Track the selection for later use.
        stmt_state.exec_statement_selection.set(columns).unwrap();

        // Plan each child
        for arg in &stmt_state.args {
            let Arg::Sub { stmt_id, .. } = arg else {
                continue;
            };

            self.plan_stmt_materialization(*stmt_id);
        }

        // Plans a NestedMerge if one is needed
        let output_node_id = if let Some(materialize_nested_merge) = self.plan_nested_merge(stmt_id)
        {
            let materialize_nested_merge_id = self.graph.nodes.len();
            self.graph.nodes.push(materialize_nested_merge);
            materialize_nested_merge_id
        } else {
            // Plan the final projection to handle the returning clause.
            let project_node_id = self.graph.nodes.len();
            self.graph.nodes.push(
                MaterializationKind::Project {
                    input: exec_stmt_node_id,
                    projection: returning,
                }
                .into(),
            );
            project_node_id
        };

        stmt_state.output.set(Some(output_node_id));
    }

    fn plan_nested_merge(&mut self, stmt_id: StmtId) -> Option<MaterializationNode> {
        let stmt_state = &self.stmts[stmt_id.0];

        // Return if there is no nested merge to do
        let need_nested_merge = stmt_state
            .args
            .iter()
            .any(|arg| matches!(arg, Arg::Sub { .. }));
        if !need_nested_merge {
            return None;
        }

        let planner = PlanNestedMerge {
            schema: self.schema,
            stmts: self.stmts,
            graph: &mut self.graph,
            inputs: IndexSet::new(),
            stack: vec![],
        };
        Some(planner.plan_nested_merge(stmt_id))
    }

    fn compute_execution_order(&mut self, exit: NodeId) {
        debug_assert!(self.graph.execution_order.is_empty());
        // Backward traversal to mark reachable nodes
        let mut stack = vec![exit];
        self.graph.nodes[exit].visited.set(true);

        while let Some(node_id) = stack.pop() {
            self.graph.execution_order.push(node_id);

            fn visit(graph: &MaterializationGraph, stack: &mut Vec<NodeId>, node_id: NodeId) {
                if !graph.nodes[node_id].visited.get() {
                    graph.nodes[node_id].visited.set(true);
                    stack.push(node_id);
                }
            }

            match &self.graph.nodes[node_id].kind {
                MaterializationKind::ExecStatement { inputs, .. } => {
                    for &input_id in inputs {
                        visit(&self.graph, &mut stack, input_id);
                    }
                }
                MaterializationKind::NestedMerge { inputs, .. } => {
                    for &input_id in inputs {
                        visit(&self.graph, &mut stack, input_id);
                    }
                }
                MaterializationKind::Project { input, .. } => {
                    visit(&self.graph, &mut stack, *input);
                }
            }
        }

        self.graph.execution_order.reverse();
    }
}

impl MaterializationGraph {
    fn new() -> MaterializationGraph {
        MaterializationGraph {
            nodes: vec![],
            execution_order: vec![],
        }
    }
}

impl From<MaterializationKind> for MaterializationNode {
    fn from(value: MaterializationKind) -> Self {
        MaterializationNode {
            kind: value,
            var: Cell::new(None),
            visited: Cell::new(false),
        }
    }
}

impl PlanNestedMerge<'_> {
    fn plan_nested_merge(mut self, root: StmtId) -> MaterializationNode {
        self.stack.push(root);
        let root = self.plan_nested_level(root, 0);
        self.stack.pop();

        MaterializationKind::NestedMerge {
            inputs: self.inputs,
            root,
        }
        .into()
    }

    fn plan_nested_child(&mut self, stmt_id: StmtId, depth: usize) -> plan::NestedChild {
        self.stack.push(stmt_id);

        let level = self.plan_nested_level(stmt_id, depth);
        let stmt_state = &self.stmts[stmt_id.0];
        let selection = stmt_state.exec_statement_selection.get().unwrap();

        let select = stmt_state
            .stmt
            .as_deref()
            .unwrap()
            .as_query()
            .unwrap()
            .body
            .as_select();

        // Extract the qualification. For now, we will just re-run the
        // entire where clause, but that can be improved later.
        let mut filter = select.filter.clone();

        visit_mut::for_each_expr_mut(&mut filter, |expr| match expr {
            stmt::Expr::Arg(expr_arg) => {
                let Arg::Ref { nesting, index, .. } = &stmt_state.args[expr_arg.position] else {
                    todo!()
                };

                debug_assert!(*nesting > 0);

                *expr = stmt::Expr::arg_project(depth - *nesting, [*index]);
            }
            stmt::Expr::Reference(expr_reference) => {
                let index = selection.get_index_of(expr_reference).unwrap();
                *expr = stmt::Expr::arg_project(depth, [index]);
            }
            _ => {}
        });

        let filter_arg_tys = self.build_filter_arg_tys();
        let filter = eval::Func::from_stmt(filter, filter_arg_tys);

        let ret = plan::NestedChild {
            level,
            qualification: plan::MergeQualification::Predicate(filter),
        };

        self.stack.pop();

        ret
    }

    fn plan_nested_level(&mut self, stmt_id: StmtId, depth: usize) -> NestedLevel {
        let stmt_state = &self.stmts[stmt_id.0];
        let selection = stmt_state.exec_statement_selection.get().unwrap();

        // First, track the batch-load as a required input for the nested merge
        let (source, _) = self
            .inputs
            .insert_full(stmt_state.exec_statement.get().unwrap());

        let select = stmt_state
            .stmt
            .as_deref()
            .unwrap()
            .as_query()
            .unwrap()
            .body
            .as_select();

        let mut nested = vec![];

        // Map the returning clause to projection expression
        let mut projection = select.returning.as_expr().clone();

        visit_mut::for_each_expr_mut(&mut projection, |expr| match expr {
            stmt::Expr::Arg(expr_arg) => match &stmt_state.args[expr_arg.position] {
                Arg::Sub { stmt_id, .. } => {
                    let nested_child = self.plan_nested_child(*stmt_id, depth + 1);
                    nested.push(nested_child);

                    // Taking the
                    *expr = stmt::Expr::arg(nested.len());
                }
                Arg::Ref { .. } => todo!(),
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

    fn build_projection_arg_tys(&self, nested_children: &[plan::NestedChild]) -> Vec<stmt::Type> {
        let curr = self.stack.last().unwrap();
        let mut ret = vec![self.build_exec_statement_ty_for(*curr)];

        for nested in nested_children {
            ret.push(stmt::Type::list(nested.level.projection.ret.clone()));
        }

        ret
    }

    fn build_exec_statement_ty_for(&self, stmt_id: StmtId) -> stmt::Type {
        let stmt_state = &self.stmts[stmt_id.0];
        let cx =
            stmt::ExprContext::new_with_target(self.schema, stmt_state.stmt.as_deref().unwrap());

        let mut fields = vec![];

        for expr_reference in stmt_state.exec_statement_selection.get().unwrap() {
            fields.push(cx.infer_expr_reference_ty(expr_reference));
        }

        stmt::Type::Record(fields)
    }
}
