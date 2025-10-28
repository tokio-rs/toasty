mod materialize_nested_merge;

use std::{cell::Cell, ops};

use index_vec::IndexVec;
use indexmap::{indexset, IndexSet};
use toasty_core::{
    schema::db::{IndexId, TableId},
    stmt::{self, visit, visit_mut},
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

    /// Nodes that must execute *before* the current one. This should be a
    /// superset of the node's inputs.
    pub(crate) deps: IndexSet<NodeId>,

    /// Variable where the output is stored
    pub(crate) var: Cell<Option<plan::VarId>>,

    /// Number of nodes that use this one as input.
    pub(crate) num_uses: Cell<usize>,

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

    /// Find primary keys by index
    FindPkByIndex(MaterializeFindPkByIndex),

    /// Get records by primary key
    GetByKey(MaterializeGetByKey),

    /// Execute a nested merge
    NestedMerge(MaterializeNestedMerge),

    /// Projection operation - transforms records
    Project(MaterializeProject),

    QueryPk(MaterializeQueryPk),
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

    /// Node return type
    pub(crate) ty: stmt::Type,
}

#[derive(Debug)]
pub(crate) struct MaterializeFilter {
    /// Input needed to reify the statement
    pub(crate) input: NodeId,

    /// Filter
    pub(crate) filter: eval::Func,

    /// Row type
    pub(crate) ty: stmt::Type,
}

#[derive(Debug)]
pub(crate) struct MaterializeFindPkByIndex {
    pub(crate) inputs: IndexSet<NodeId>,
    pub(crate) table: TableId,
    pub(crate) index: IndexId,
    pub(crate) filter: stmt::Expr,
    pub(crate) ty: stmt::Type,
}

#[derive(Debug)]
pub(crate) struct MaterializeGetByKey {
    /// Keys are always specified as an input, whether const or a set of
    /// dependent materializations and transformations.
    pub(crate) input: NodeId,

    /// The table to get keys from
    pub(crate) table: TableId,

    /// Columns to get
    pub(crate) columns: IndexSet<stmt::ExprReference>,

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
    pub(crate) projection: eval::Func,

    pub(crate) ty: stmt::Type,
}

#[derive(Debug)]
pub(crate) struct MaterializeQueryPk {
    pub(crate) table: TableId,

    /// Columns to get
    pub(crate) columns: IndexSet<stmt::ExprReference>,

    /// How to filter the index
    pub(crate) pk_filter: stmt::Expr,

    /// Additional filter to pass to the database
    pub(crate) row_filter: Option<stmt::Expr>,

    pub(crate) ty: stmt::Type,
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

        // First, plan dependency statements. These are statments that must run
        // before the current one but do not reference the current statement.
        for &stmt_id in &stmt_info.deps {
            self.plan_materialize_statement(stmt_id);
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

                visit_mut::for_each_expr_mut(&mut filter, |expr| {
                    match expr {
                        stmt::Expr::Reference(stmt::ExprReference::Column(expr_column)) => {
                            debug_assert_eq!(0, expr_column.nesting);
                            // We need to up the nesting to reflect that the filter is moved
                            // one level deeper.
                            expr_column.nesting += 1;
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
                visit_mut::for_each_expr_mut(&mut stmt.filter_mut(), |expr| match expr {
                    stmt::Expr::Reference(stmt::ExprReference::Column(expr_column)) => {
                        debug_assert_eq!(0, expr_column.nesting);
                    }
                    stmt::Expr::Arg(expr_arg) => {
                        let Arg::Ref {
                            batch_load_index: index,
                            ..
                        } = &stmt_info.args[expr_arg.position]
                        else {
                            todo!()
                        };

                        *expr = stmt::Expr::arg_project(0, [*index]);
                    }
                    _ => {}
                });
            }
        }

        let exec_stmt_node_id = if stmt.filter_or_default().is_false() {
            debug_assert!(stmt_info.deps.is_empty(), "TODO");
            // Don't bother querying and just return false
            self.insert_const(vec![], self.engine.infer_record_list_ty(&stmt, &columns))
        } else if self.engine.capability().sql {
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

            // With SQL capability, we can just punt the details of execution to
            // the database's query planner.
            self.graph.insert_with_deps(
                MaterializeExecStatement { inputs, stmt, ty },
                stmt_info.dependent_materializations(&self.store),
            )
        } else {
            // Without SQL capability, we have to plan the materialization of
            // the statement based on available indices.
            let mut index_plan = self.engine.plan_index_path2(&stmt);
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
                    vec![]
                };

                // If using the primary key to find rows, try to convert the
                // filter expression to a set of primary-key keys.
                //
                // TODO: move this to the index planner?
                let cx = self.engine.expr_cx_for(&stmt);
                pk_keys = self.engine.try_build_key_filter2(
                    cx,
                    index_plan.index,
                    &index_plan.index_filter,
                    pk_keys_project_args,
                );
            };

            // If fetching rows using GetByKey, some databases do not support
            // applying additional filters to the rows before returning results.
            // In this case, the result_filter needs to be applied in-memory.
            if pk_keys.is_some() || !index_plan.index.primary_key {
                if let Some(result_filter) = index_plan.result_filter.take() {
                    post_filter = Some(match post_filter {
                        Some(post_filter) => stmt::Expr::and(result_filter, post_filter),
                        None => result_filter,
                    });
                }
            }

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
            let ty = self.engine.infer_record_list_ty(&stmt, &columns);

            // Type of the index key. Value for single index keys, record for
            // composite.
            let index_key_ty = stmt::Type::list(self.engine.index_key_record_ty(index_plan.index));

            let mut node_id = if index_plan.index.primary_key {
                // TODO: I'm not sure if calling try_build_key_filter is the
                // right way to do this anymore, but it works for now?
                if let Some(keys) = pk_keys {
                    let get_by_key_input = if ref_source.is_none() {
                        self.insert_const(keys.eval_const(), index_key_ty)
                    } else if keys.is_identity() {
                        debug_assert_eq!(1, inputs.len(), "TODO");
                        inputs[0]
                    } else {
                        let ty = stmt::Type::list(keys.ret.clone());
                        // Gotta project
                        self.graph.insert(MaterializeProject {
                            input: inputs[0],
                            projection: keys,
                            ty,
                        })
                    };

                    self.graph.insert(MaterializeGetByKey {
                        input: get_by_key_input,
                        table: table_id,
                        columns: columns.clone(),
                        ty: ty.clone(),
                    })
                } else {
                    assert!(inputs.is_empty(), "TODO");
                    assert!(ref_source.is_none(), "TODO");

                    self.graph.insert(MaterializeQueryPk {
                        table: table_id,
                        columns: columns.clone(),
                        pk_filter: index_plan.index_filter,
                        row_filter: index_plan.result_filter,
                        ty: ty.clone(),
                    })
                }
            } else {
                assert!(index_plan.post_filter.is_none(), "TODO");
                assert!(inputs.len() <= 1, "TODO: inputs={inputs:#?}");

                // Args not supportd yet...
                visit::for_each_expr(&index_plan.index_filter, |expr| {
                    if let stmt::Expr::Arg(expr_arg) = expr {
                        debug_assert_eq!(0, expr_arg.position, "TODO; index_plan={index_plan:#?}");
                        debug_assert_eq!(Some(expr_arg), ref_source.as_ref());
                    }
                });

                let get_by_key_input = self.graph.insert(MaterializeFindPkByIndex {
                    inputs,
                    table: index_plan.index.on,
                    index: index_plan.index.id,
                    filter: index_plan.index_filter.take(),
                    ty: index_key_ty,
                });

                self.graph.insert(MaterializeGetByKey {
                    input: get_by_key_input,
                    table: table_id,
                    columns: columns.clone(),
                    ty: ty.clone(),
                })
            };

            // If there is a post filter, we need to apply a filter step on the returned rows.
            if let Some(post_filter) = post_filter {
                let item_ty = ty.unwrap_list_ref();
                node_id = self.graph.insert(MaterializeFilter {
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

            let project_node_id = self.graph.insert(MaterializeProject {
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
            let Arg::Sub { stmt_id, .. } = arg else {
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
                        .insert_with_deps(MaterializeConst { value: rows, ty }, [exec_stmt_node_id])
                }
                stmt::Returning::Expr(returning) => {
                    let arg_ty = self.graph[exec_stmt_node_id].ty().unwrap_list_ref().clone();
                    let projection = eval::Func::from_stmt(returning, vec![arg_ty]);
                    let ty = stmt::Type::list(projection.ret.clone());

                    // Plan the final projection to handle the returning clause.
                    self.graph.insert(MaterializeProject {
                        input: exec_stmt_node_id,
                        projection,
                        ty,
                    })
                }
                returning => panic!("unexpected `stmt::Returning` kind; returning={returning:#?}"),
            }
        } else {
            exec_stmt_node_id
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

            for &dep_id in &self.graph[node_id].deps {
                let dep = &self.graph[dep_id];

                // Increment use count
                dep.num_uses.set(dep.num_uses.get() + 1);

                if !dep.visited.get() {
                    dep.visited.set(true);
                    stack.push(dep_id);
                }
            }
        }

        self.graph.execution_order.reverse();
    }

    #[track_caller]
    fn insert_const(&mut self, value: impl Into<stmt::Value>, ty: stmt::Type) -> NodeId {
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

        self.graph.insert(MaterializeConst {
            value: value.unwrap_list(),
            ty,
        })
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

    pub(super) fn insert_with_deps<I>(
        &mut self,
        node: impl Into<MaterializeNode>,
        deps: I,
    ) -> NodeId
    where
        I: IntoIterator<Item = NodeId>,
    {
        let mut node = node.into();
        node.deps.extend(deps);
        self.store.push(node)
    }

    pub(super) fn var_id(&self, node_id: NodeId) -> plan::VarId {
        self.store[node_id].var_id()
    }

    pub(super) fn ty(&self, node_id: NodeId) -> &stmt::Type {
        self.store[node_id].ty()
    }
}

impl MaterializeNode {
    pub(super) fn ty(&self) -> &stmt::Type {
        match &self.kind {
            MaterializeKind::Const(kind) => &kind.ty,
            MaterializeKind::ExecStatement(kind) => &kind.ty,
            MaterializeKind::Filter(kind) => &kind.ty,
            MaterializeKind::FindPkByIndex(kind) => &kind.ty,
            MaterializeKind::GetByKey(kind) => &kind.ty,
            MaterializeKind::QueryPk(kind) => &kind.ty,
            MaterializeKind::Project(kind) => &kind.ty,
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

impl From<MaterializeFindPkByIndex> for MaterializeNode {
    fn from(value: MaterializeFindPkByIndex) -> Self {
        MaterializeKind::FindPkByIndex(value).into()
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

impl From<MaterializeQueryPk> for MaterializeNode {
    fn from(value: MaterializeQueryPk) -> Self {
        MaterializeKind::QueryPk(value).into()
    }
}

impl From<MaterializeKind> for MaterializeNode {
    fn from(value: MaterializeKind) -> Self {
        let deps = match &value {
            MaterializeKind::Const(_materialize_const) => IndexSet::new(),
            MaterializeKind::ExecStatement(materialize_exec_statement) => {
                materialize_exec_statement.inputs.clone()
            }
            MaterializeKind::Filter(materialize_filter) => indexset![materialize_filter.input],
            MaterializeKind::FindPkByIndex(materialize_find_pk_by_index) => {
                materialize_find_pk_by_index.inputs.clone()
            }
            MaterializeKind::GetByKey(materialize_get_by_key) => {
                indexset![materialize_get_by_key.input]
            }
            MaterializeKind::NestedMerge(materialize_nested_merge) => {
                materialize_nested_merge.inputs.clone()
            }
            MaterializeKind::Project(materialize_project) => indexset![materialize_project.input],
            MaterializeKind::QueryPk(_materialize_query_pk) => IndexSet::new(),
        };

        MaterializeNode {
            kind: value,
            deps,
            var: Cell::new(None),
            num_uses: Cell::new(0),
            visited: Cell::new(false),
        }
    }
}
