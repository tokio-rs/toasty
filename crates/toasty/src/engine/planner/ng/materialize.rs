mod materialize_nested_merge;

use std::{cell::Cell, ops};

use index_vec::IndexVec;
use indexmap::IndexSet;
use toasty_core::{
    schema::db::{ColumnId, TableId},
    stmt::{self, visit_mut},
};

use crate::engine::{
    eval, plan,
    planner::ng::{Arg, StatementInfoStore, StmtId},
    Engine,
};

#[derive(Debug)]
pub(crate) struct MaterializeGraph {
    /// Nodes in the graph
    pub(crate) store: IndexVec<NodeId, MaterializeNode>,

    /// Order of execution
    pub(crate) execution_order: Vec<NodeId>,
}

#[derive(Debug)]
pub(crate) struct MaterializeNode {
    /// Materialization kind
    pub(crate) kind: MaterializeKind,

    /// Variable where the output is stored
    pub(crate) var: Cell<Option<plan::VarId>>,

    /// Used for topo-sort
    visited: Cell<bool>,
}

index_vec::define_index_type! {
    pub(crate) struct NodeId = u32;
}

/// Materialization operation
#[derive(Debug)]
pub(crate) enum MaterializeKind {
    /// A constant value
    Const(MaterializeConst),

    /// Execute a database query
    ExecStatement(MaterializeExecStatement),

    /// Filter results
    Filter(MaterializeFilter),

    /// Get records by primary key
    GetByKey(MaterializeGetByKey),

    /// Execute a nested merge
    NestedMerge(MaterializeNestedMerge),

    /// Projection operation - transforms records
    Project(MaterializeProject),
}

#[derive(Debug)]
pub(crate) struct MaterializeConst {
    pub(crate) value: Vec<stmt::Value>,
    pub(crate) ty: stmt::Type,
}

#[derive(Debug)]
pub(crate) struct MaterializeExecStatement {
    /// Inputs needed to reify the statement
    pub(crate) inputs: IndexSet<NodeId>,

    /// The database query to execute
    pub(crate) stmt: stmt::Statement,
}

#[derive(Debug)]
pub(crate) struct MaterializeFilter {
    /// Input needed to reify the statement
    pub(crate) input: NodeId,

    /// Filter
    pub(crate) filter: eval::Func,
}

#[derive(Debug)]
pub(crate) struct MaterializeGetByKey {
    /// Inputs needed to reify the operation
    pub(crate) inputs: IndexSet<NodeId>,

    /// The table to get keys from
    pub(crate) table: TableId,

    /// Columns to get
    pub(crate) columns: IndexSet<stmt::ExprReference>,

    /// Keys to get
    pub(crate) keys: eval::Func,

    /// Return type
    pub(crate) ty: stmt::Type,
}

#[derive(Debug)]
pub(crate) struct MaterializeNestedMerge {
    /// Inputs needed to reify the statement
    pub(crate) inputs: IndexSet<NodeId>,

    /// The root nested merge level
    pub(crate) root: plan::NestedLevel,
}

#[derive(Debug)]
pub(crate) struct MaterializeProject {
    /// Input required to perform the projection
    pub(crate) input: NodeId,

    /// Projection expression
    pub(crate) projection: stmt::Expr,
}

#[derive(Debug)]
struct MaterializePlanner<'a> {
    engine: &'a Engine,

    /// Root statement and all nested statements.
    store: &'a StatementInfoStore,

    /// Graph of operations needed to materialize the statement, in-progress
    graph: &'a mut MaterializeGraph,
}

impl super::PlannerNg<'_, '_> {
    pub(super) fn plan_materializations(&mut self) {
        MaterializePlanner {
            engine: self.old.engine,
            store: &self.store,
            graph: &mut self.graph,
        }
        .plan_materialize();
    }
}

impl MaterializePlanner<'_> {
    fn plan_materialize(&mut self) {
        let root_id = self.store.root_id();
        self.plan_materialize_statement(root_id);

        let exit = self.store.root().output.get().unwrap();
        self.compute_materialization_execution_order(exit);
    }

    fn plan_materialize_statement(&mut self, stmt_id: StmtId) {
        let stmt_info = &self.store[stmt_id];
        let mut stmt = stmt_info.stmt.as_deref().unwrap().clone();

        // Get the returning clause
        let stmt::Statement::Query(query) = &mut stmt else {
            panic!()
        };

        // Tracks if the query is a single query
        let single = query.single;
        query.single = false;

        let stmt::ExprSet::Select(select) = &mut query.body else {
            panic!()
        };

        let filter_is_false = select.filter.is_false();

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
                    let (index, _) = columns.insert_full(*expr_reference);
                    *expr = stmt::Expr::arg_project(0, [index]);
                }
                stmt::Expr::Arg(expr_arg) => match &stmt_info.args[expr_arg.position] {
                    Arg::Ref { .. } => {
                        todo!("refs in returning is not yet supported");
                    }
                    Arg::Sub { stmt_id, input } => {
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
                },
                _ => {}
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
            let Arg::Ref {
                stmt_id: target_id,
                input,
                ..
            } = arg
            else {
                continue;
            };

            assert!(
                self.engine.capability().sql,
                "TODO: only supported on sql right now"
            );
            assert!(ref_source.is_none(), "TODO: handle more complex ref cases");
            assert!(!filter_is_false, "TODO: handle const false filters");

            // Find the back-ref for this arg
            let node_id = self.store[target_id].back_refs[&stmt_id]
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
                        let Arg::Ref {
                            input,
                            batch_load_index: index,
                            ..
                        } = &stmt_info.args[expr_arg.position]
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

        let exec_stmt_node_id = if filter_is_false {
            // Don't bother querying and just return false
            self.graph.insert(MaterializeConst {
                value: vec![],
                ty: self.engine.infer_record_list_ty(&stmt, &columns),
            })
        } else if self.engine.capability().sql {
            select.returning = stmt::Returning::Expr(stmt::Expr::record(
                columns
                    .iter()
                    .map(|expr_reference| stmt::Expr::from(*expr_reference)),
            ));

            // With SQL capability, we can just punt the details of execution to
            // the database's query planner.
            self.graph.insert(MaterializeExecStatement { inputs, stmt })
        } else {
            // Without SQL capability, we have to plan the materialization of
            // the statement based on available indices.
            let index_plan = self.engine.plan_index_path2(&stmt);

            let mut node_id = None;

            // TODO: handle non-PK indexes
            assert!(index_plan.index.primary_key);

            if let Some(keys) = self.engine.try_build_key_filter2(index_plan.index, &stmt) {
                // If there is a result filter, we will need to evaluate it
                // *after* fetching the rows, so we need to make sure that any
                // column referenced in the filter is fetched.
                let mut result_filter = index_plan.result_filter.clone();

                if let Some(result_filter) = &mut result_filter {
                    visit_mut::for_each_expr_mut(result_filter, |expr| match expr {
                        stmt::Expr::Reference(expr_reference) => {
                            let (index, _) = columns.insert_full(*expr_reference);
                            *expr = stmt::Expr::arg_project(0, [index]);
                        }
                        stmt::Expr::Arg(_) => todo!("expr={expr:#?}"),
                        _ => {}
                    });
                }

                let ty = self.engine.infer_record_list_ty(&stmt, &columns);

                node_id = Some(self.graph.insert(MaterializeGetByKey {
                    inputs,
                    table: self.engine.resolve_table_for(&stmt).id,
                    // TODO: don't clone
                    columns: columns.clone(),
                    keys,
                    ty,
                }));

                // If there is a result filter, we need to apply a filter step on the returned rows.

                if let Some(result_filter) = result_filter {
                    todo!("result_filter={result_filter:#?}");
                }
            }

            node_id.expect("bug")
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

            let project_node_id = self.graph.insert(MaterializeProject {
                input: exec_stmt_node_id,
                projection,
            });
            back_ref.node_id.set(Some(project_node_id));
        }

        // Track the selection for later use.
        stmt_info.exec_statement_selection.set(columns).unwrap();

        // Plan each child
        for arg in &stmt_info.args {
            let Arg::Sub { stmt_id, .. } = arg else {
                continue;
            };

            self.plan_materialize_statement(*stmt_id);
        }

        // Plans a NestedMerge if one is needed
        let output_node_id = if let Some(node_id) = self.plan_nested_merge(stmt_id) {
            node_id
        } else {
            debug_assert!(
                !single || ref_source.is_some(),
                "TODO: single queries not supported here"
            );

            // Plan the final projection to handle the returning clause.
            self.graph.insert(MaterializeProject {
                input: exec_stmt_node_id,
                projection: returning,
            })
        };

        stmt_info.output.set(Some(output_node_id));
    }

    fn compute_materialization_execution_order(&mut self, exit: NodeId) {
        debug_assert!(self.graph.execution_order.is_empty());
        // Backward traversal to mark reachable nodes
        let mut stack = vec![exit];
        self.graph[exit].visited.set(true);

        while let Some(node_id) = stack.pop() {
            self.graph.execution_order.push(node_id);

            fn visit(graph: &MaterializeGraph, stack: &mut Vec<NodeId>, node_id: NodeId) {
                if !graph[node_id].visited.get() {
                    graph[node_id].visited.set(true);
                    stack.push(node_id);
                }
            }

            match &self.graph[node_id].kind {
                MaterializeKind::Const(_) => {}
                MaterializeKind::ExecStatement(materialize_exec_statement) => {
                    for &input_id in &materialize_exec_statement.inputs {
                        visit(&self.graph, &mut stack, input_id);
                    }
                }
                MaterializeKind::Filter(materialize_filter) => {
                    visit(&self.graph, &mut stack, materialize_filter.input);
                }
                MaterializeKind::GetByKey(materialize_get_by_key) => {
                    for &input_id in &materialize_get_by_key.inputs {
                        visit(&self.graph, &mut stack, input_id);
                    }
                }
                MaterializeKind::NestedMerge(materialize_nested_merge) => {
                    for &input_id in &materialize_nested_merge.inputs {
                        visit(&self.graph, &mut stack, input_id);
                    }
                }
                MaterializeKind::Project(materialize_project) => {
                    visit(&self.graph, &mut stack, materialize_project.input);
                }
            }
        }

        self.graph.execution_order.reverse();
    }
}

impl MaterializeGraph {
    pub(super) fn new() -> MaterializeGraph {
        MaterializeGraph {
            store: IndexVec::new(),
            execution_order: vec![],
        }
    }

    /// Insert a node into the graph
    pub(super) fn insert(&mut self, node: impl Into<MaterializeNode>) -> NodeId {
        self.store.push(node.into())
    }

    /// Returns the type the node evaluates to
    pub(super) fn ty(&self, node_id: NodeId) -> &stmt::Type {
        self.store[node_id].ty()
    }

    pub(super) fn var_id(&self, node_id: NodeId) -> plan::VarId {
        self.store[node_id].var_id()
    }
}

impl MaterializeNode {
    pub(super) fn ty(&self) -> &stmt::Type {
        match &self.kind {
            MaterializeKind::Const(materialize_const) => &materialize_const.ty,
            MaterializeKind::GetByKey(materialize_get_by_key) => &materialize_get_by_key.ty,
            _ => todo!("node={self:#?}"),
        }
    }

    pub(super) fn var_id(&self) -> plan::VarId {
        self.var.get().unwrap()
    }
}

impl ops::Index<NodeId> for MaterializeGraph {
    type Output = MaterializeNode;

    fn index(&self, index: NodeId) -> &Self::Output {
        self.store.index(index)
    }
}

impl ops::IndexMut<NodeId> for MaterializeGraph {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        self.store.index_mut(index)
    }
}

impl ops::Index<&NodeId> for MaterializeGraph {
    type Output = MaterializeNode;

    fn index(&self, index: &NodeId) -> &Self::Output {
        self.store.index(*index)
    }
}

impl ops::IndexMut<&NodeId> for MaterializeGraph {
    fn index_mut(&mut self, index: &NodeId) -> &mut Self::Output {
        self.store.index_mut(*index)
    }
}

impl From<MaterializeConst> for MaterializeNode {
    fn from(value: MaterializeConst) -> Self {
        MaterializeKind::Const(value).into()
    }
}

impl From<MaterializeExecStatement> for MaterializeNode {
    fn from(value: MaterializeExecStatement) -> Self {
        MaterializeKind::ExecStatement(value).into()
    }
}

impl From<MaterializeFilter> for MaterializeNode {
    fn from(value: MaterializeFilter) -> Self {
        MaterializeKind::Filter(value).into()
    }
}

impl From<MaterializeGetByKey> for MaterializeNode {
    fn from(value: MaterializeGetByKey) -> Self {
        MaterializeKind::GetByKey(value).into()
    }
}

impl From<MaterializeNestedMerge> for MaterializeNode {
    fn from(value: MaterializeNestedMerge) -> Self {
        MaterializeKind::NestedMerge(value).into()
    }
}

impl From<MaterializeProject> for MaterializeNode {
    fn from(value: MaterializeProject) -> Self {
        MaterializeKind::Project(value).into()
    }
}

impl From<MaterializeKind> for MaterializeNode {
    fn from(value: MaterializeKind) -> Self {
        MaterializeNode {
            kind: value,
            var: Cell::new(None),
            visited: Cell::new(false),
        }
    }
}
