use std::cell::{Cell, OnceCell};

use indexmap::IndexSet;
use toasty_core::stmt::{self, visit_mut};

use crate::engine::{
    plan,
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

    /// Projection operation - transforms records
    Project {
        /// Inputs required to perform the projection
        inputs: IndexSet<NodeId>,

        /// Projection expression
        projection: stmt::Expr,
    },
}

#[derive(Debug)]
struct PlanMaterialization<'a> {
    /// Root statement and all nested statements.
    stmts: &'a [StatementState],

    /// Graph of operations needed to materialize the statement, in-progress
    graph: MaterializationGraph,
}

impl super::Planner<'_> {
    pub(super) fn plan_materializations(
        &self,
        stmts: &[StatementState],
        root: StmtId,
    ) -> MaterializationGraph {
        let mut plan_materialization = PlanMaterialization {
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
                    *expr = stmt::Expr::arg(index);
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

        // If there are any ref args, then the statement needs to be rewritten
        // to batch load all records for a NestedMerge operation .
        let mut ref_source = None;

        for arg in &self.stmts[stmt_id.0].args {
            let Arg::Ref { stmt_id, input, .. } = arg else {
                continue;
            };

            assert!(ref_source.is_none(), "TODO: handle more complex ref cases");

            let node_id = self.stmts[stmt_id.0].exec_statement.get().unwrap();
            let (index, _) = inputs.insert_full(node_id);
            ref_source = Some(stmt::Values::from(stmt::Expr::arg(index)));
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

            let sub_select = stmt::Select::new(ref_source, select.filter.take());
            select.filter = stmt::Expr::exists(stmt::Query::builder(sub_select).returning(1));
        }

        // Create the exec statement materialization node.
        let exec_stmt_node_id = self.graph.nodes.len();
        self.graph
            .nodes
            .push(MaterializationKind::ExecStatement { inputs, stmt }.into());

        // Track the exec statement materialization node.
        stmt_state.exec_statement.set(Some(exec_stmt_node_id));

        // Plan each child
        for arg in &stmt_state.args {
            let Arg::Sub { stmt_id, .. } = arg else {
                continue;
            };

            self.plan_stmt_materialization(*stmt_id);
        }

        // Plans a NestedMerge if one is needed
        self.plan_nested_merge(stmt_id);

        // Plan the final projection to handle the returning clause.
        let project_node_id = self.graph.nodes.len();
        self.graph.nodes.push(
            MaterializationKind::Project {
                inputs: [exec_stmt_node_id].into(),
                projection: returning,
            }
            .into(),
        );
        stmt_state.output.set(Some(project_node_id));
    }

    fn plan_nested_merge(&mut self, stmt_id: StmtId) {
        let stmt_state = &self.stmts[stmt_id.0];

        for arg in &stmt_state.args {
            if matches!(arg, Arg::Sub { .. }) {
                todo!("IMPLEMENT NESTED MERGE");
            }
        }
    }

    fn compute_execution_order(&mut self, exit: NodeId) {
        debug_assert!(self.graph.execution_order.is_empty());
        // Backward traversal to mark reachable nodes
        let mut stack = vec![exit];
        self.graph.nodes[exit].visited.set(true);

        while let Some(node_id) = stack.pop() {
            self.graph.execution_order.push(node_id);

            let inputs = match &self.graph.nodes[node_id].kind {
                MaterializationKind::ExecStatement { inputs, .. } => inputs,
                MaterializationKind::Project { inputs, .. } => inputs,
            };

            for &input_id in inputs {
                if !self.graph.nodes[input_id].visited.get() {
                    self.graph.nodes[input_id].visited.set(true);
                    stack.push(input_id);
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
