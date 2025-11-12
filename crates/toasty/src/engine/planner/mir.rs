mod r#const;
pub(crate) use r#const::Const;

mod exec_statement;
pub(crate) use exec_statement::ExecStatement;

use std::{cell::Cell, ops};

use index_vec::IndexVec;
use indexmap::{indexset, IndexSet};
use toasty_core::{
    schema::db::{IndexId, TableId},
    stmt,
};

use crate::engine::{eval, exec};

#[derive(Debug)]
pub(crate) struct Node {
    /// Materialization kind
    pub(crate) op: Operation,

    /// Nodes that must execute *before* the current one. This should be a
    /// superset of the node's inputs.
    pub(crate) deps: IndexSet<NodeId>,

    /// Variable where the output is stored
    pub(crate) var: Cell<Option<exec::VarId>>,

    /// Number of nodes that use this one as input.
    pub(crate) num_uses: Cell<usize>,

    /// Used for topo-sort
    pub(crate) visited: Cell<bool>,
}

/// Materialization operation
#[derive(Debug)]
pub(crate) enum Operation {
    /// A constant value
    Const(Const),

    DeleteByKey(DeleteByKey),

    /// Execute a database query
    ExecStatement(Box<ExecStatement>),

    /// Filter results
    Filter(Filter),

    /// Find primary keys by index
    FindPkByIndex(FindPkByIndex),

    /// Get records by primary key
    GetByKey(GetByKey),

    /// Execute a nested merge
    NestedMerge(NestedMerge),

    /// Projection operation - transforms records
    Project(Project),

    /// Read-modify-write. The write only succeeds if the values read are not
    /// modified.
    ReadModifyWrite(Box<ReadModifyWrite>),

    QueryPk(QueryPk),

    UpdateByKey(UpdateByKey),
}

#[derive(Debug)]
pub(crate) struct DeleteByKey {
    /// Keys are always specified as an input, whether const or a set of
    /// dependent materializations and transformations.
    pub(crate) input: NodeId,

    /// The table to get keys from
    pub(crate) table: TableId,

    pub(crate) filter: Option<stmt::Expr>,

    /// Return type
    pub(crate) ty: stmt::Type,
}

#[derive(Debug)]
pub(crate) struct Filter {
    /// Input needed to reify the statement
    pub(crate) input: NodeId,

    /// Filter
    pub(crate) filter: eval::Func,

    /// Row type
    pub(crate) ty: stmt::Type,
}

#[derive(Debug)]
pub(crate) struct FindPkByIndex {
    pub(crate) inputs: IndexSet<NodeId>,
    pub(crate) table: TableId,
    pub(crate) index: IndexId,
    pub(crate) filter: stmt::Expr,
    pub(crate) ty: stmt::Type,
}

#[derive(Debug)]
pub(crate) struct GetByKey {
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
pub(crate) struct NestedMerge {
    /// Inputs needed to reify the statement
    pub(crate) inputs: IndexSet<NodeId>,

    /// The root nested merge level
    pub(crate) root: exec::NestedLevel,
}

#[derive(Debug)]
pub(crate) struct Project {
    /// Input required to perform the projection
    pub(crate) input: NodeId,

    /// Projection expression
    pub(crate) projection: eval::Func,

    pub(crate) ty: stmt::Type,
}

#[derive(Debug)]
pub(crate) struct ReadModifyWrite {
    /// Inputs needed to reify the statement
    pub(crate) inputs: IndexSet<NodeId>,

    /// The read statement
    pub(crate) read: stmt::Query,

    /// The write statement
    pub(crate) write: stmt::Statement,

    /// Node return type
    pub(crate) ty: stmt::Type,
}

#[derive(Debug)]
pub(crate) struct QueryPk {
    pub(crate) input: Option<NodeId>,

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
pub(crate) struct UpdateByKey {
    pub(crate) input: NodeId,

    pub(crate) table: TableId,

    pub(crate) assignments: stmt::Assignments,

    pub(crate) filter: Option<stmt::Expr>,

    pub(crate) condition: Option<stmt::Expr>,

    pub(crate) ty: stmt::Type,
}

#[derive(Debug)]
pub(crate) struct MaterializeGraph {
    /// Nodes in the graph
    pub(crate) store: IndexVec<NodeId, Node>,

    /// Order of execution
    pub(crate) execution_order: Vec<NodeId>,
}

index_vec::define_index_type! {
    pub(crate) struct NodeId = u32;
}

impl MaterializeGraph {
    pub(super) fn new() -> MaterializeGraph {
        MaterializeGraph {
            store: IndexVec::new(),
            execution_order: vec![],
        }
    }

    /// Insert a node into the graph
    pub(super) fn insert(&mut self, node: impl Into<Node>) -> NodeId {
        self.store.push(node.into())
    }

    pub(super) fn insert_with_deps<I>(&mut self, node: impl Into<Node>, deps: I) -> NodeId
    where
        I: IntoIterator<Item = NodeId>,
    {
        let mut node = node.into();
        node.deps.extend(deps);
        self.store.push(node)
    }

    pub(super) fn var_id(&self, node_id: NodeId) -> exec::VarId {
        self.store[node_id].var_id()
    }

    pub(super) fn ty(&self, node_id: NodeId) -> &stmt::Type {
        self.store[node_id].ty()
    }
}

impl Node {
    pub(super) fn ty(&self) -> &stmt::Type {
        match &self.op {
            Operation::Const(m) => &m.ty,
            Operation::DeleteByKey(m) => &m.ty,
            Operation::ExecStatement(m) => &m.ty,
            Operation::Filter(m) => &m.ty,
            Operation::FindPkByIndex(m) => &m.ty,
            Operation::GetByKey(m) => &m.ty,
            Operation::QueryPk(m) => &m.ty,
            Operation::Project(m) => &m.ty,
            Operation::UpdateByKey(m) => &m.ty,
            Operation::NestedMerge(_m) => todo!(),
            Operation::ReadModifyWrite(m) => &m.ty,
        }
    }

    pub(super) fn var_id(&self) -> exec::VarId {
        self.var.get().unwrap()
    }
}

impl ops::Index<NodeId> for MaterializeGraph {
    type Output = Node;

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
    type Output = Node;

    fn index(&self, index: &NodeId) -> &Self::Output {
        self.store.index(*index)
    }
}

impl ops::IndexMut<&NodeId> for MaterializeGraph {
    fn index_mut(&mut self, index: &NodeId) -> &mut Self::Output {
        self.store.index_mut(*index)
    }
}

impl From<DeleteByKey> for Node {
    fn from(value: DeleteByKey) -> Self {
        Operation::DeleteByKey(value).into()
    }
}

impl From<Filter> for Node {
    fn from(value: Filter) -> Self {
        Operation::Filter(value).into()
    }
}

impl From<FindPkByIndex> for Node {
    fn from(value: FindPkByIndex) -> Self {
        Operation::FindPkByIndex(value).into()
    }
}

impl From<GetByKey> for Node {
    fn from(value: GetByKey) -> Self {
        Operation::GetByKey(value).into()
    }
}

impl From<NestedMerge> for Node {
    fn from(value: NestedMerge) -> Self {
        Operation::NestedMerge(value).into()
    }
}

impl From<Project> for Node {
    fn from(value: Project) -> Self {
        Operation::Project(value).into()
    }
}

impl From<QueryPk> for Node {
    fn from(value: QueryPk) -> Self {
        Operation::QueryPk(value).into()
    }
}

impl From<UpdateByKey> for Node {
    fn from(value: UpdateByKey) -> Self {
        Operation::UpdateByKey(value).into()
    }
}

impl From<Operation> for Node {
    fn from(value: Operation) -> Self {
        let deps = match &value {
            Operation::Const(_m) => IndexSet::new(),
            Operation::DeleteByKey(m) => indexset![m.input],
            Operation::ExecStatement(m) => m.inputs.clone(),
            Operation::Filter(m) => indexset![m.input],
            Operation::FindPkByIndex(m) => m.inputs.clone(),
            Operation::GetByKey(m) => {
                indexset![m.input]
            }
            Operation::NestedMerge(m) => m.inputs.clone(),
            Operation::Project(m) => indexset![m.input],
            Operation::ReadModifyWrite(m) => m.inputs.clone(),
            Operation::QueryPk(m) => m.input.into_iter().collect(),
            Operation::UpdateByKey(m) => indexset![m.input],
        };

        Node {
            op: value,
            deps,
            var: Cell::new(None),
            num_uses: Cell::new(0),
            visited: Cell::new(false),
        }
    }
}
