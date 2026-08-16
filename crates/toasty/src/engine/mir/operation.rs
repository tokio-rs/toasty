use std::cell::Cell;

use indexmap::{IndexSet, indexset};

use crate::engine::effect;
use crate::engine::mir::Eval;

use super::{
    Alias, Const, DeleteByKey, ExecStatement, Filter, FindPkByIndex, GetByKey, NestedMerge, Node,
    NodeId, Project, QueryPk, ReadModifyWrite, Scan, UpdateByKey, Upsert,
};

/// A step in the query execution plan.
///
/// Operations represent units of work: loading data from the database,
/// filtering results, transforming records, or combining nested data.
#[derive(Debug)]
pub(crate) enum Operation {
    /// Pass another node's output through unchanged (fills a reserved slot)
    Alias(Alias),

    /// A constant value
    Const(Const),

    DeleteByKey(DeleteByKey),

    Eval(Eval),

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

    /// Full-table scan — emitted when no index covers the filter on a scan-capable driver.
    Scan(Scan),

    UpdateByKey(UpdateByKey),

    /// Atomically insert or update one record on a non-SQL database.
    Upsert(Box<Upsert>),
}

impl Operation {
    /// The operation's value edges: nodes whose outputs it reads.
    ///
    /// `Node.deps` is seeded from this set and may then be extended with
    /// ordering-only edges (e.g. "child INSERT before parent INSERT"). The
    /// ordering-only edges are derivable as `deps − inputs()`.
    pub(crate) fn inputs(&self) -> IndexSet<NodeId> {
        match self {
            Operation::Alias(m) => indexset![m.input],
            Operation::Const(_m) => IndexSet::new(),
            Operation::DeleteByKey(m) => indexset![m.input],
            Operation::Eval(m) => m.inputs.clone(),
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
            Operation::Scan(m) => m.input.into_iter().collect(),
            Operation::UpdateByKey(m) => indexset![m.input],
            Operation::Upsert(m) => m.inputs.clone(),
        }
    }

    /// The variable loads the operation's exec action performs, with
    /// multiplicity — currently one per declared input. `num_uses` refcounts
    /// are the sum of these loads across consumers, so each action must load
    /// every listed input exactly once — or release it on any path that
    /// declines the load. (A node's `guard` condition may load additional
    /// variables; those are counted separately at `LogicalPlan::new`.)
    pub(crate) fn input_loads(&self) -> impl Iterator<Item = NodeId> + use<> {
        // ReadModifyWrite declares `inputs` but its exec action asserts them
        // empty; count them anyway so a future non-empty RMW input must load
        // them rather than silently violating the counting.
        self.inputs().into_iter()
    }

    /// True for operations that write to the database.
    ///
    /// Distinct from `exec::Action::is_db_op`, which answers "issues a driver
    /// operation" for transaction wrapping — queries are db ops but not
    /// effectful. Only pure (non-effectful) nodes may be guarded, and every
    /// effectful node must be reachable from the completion node.
    pub(crate) fn is_effectful(&self) -> bool {
        match self {
            Operation::DeleteByKey(_)
            | Operation::ReadModifyWrite(_)
            | Operation::UpdateByKey(_)
            | Operation::Upsert(_) => true,

            // `classify` walks the statement tree, so a `Query` wrapping a
            // data-modifying CTE (the OCC conditional-write path) classifies
            // as mutating even though its statement kind is a read.
            Operation::ExecStatement(m) => effect::classify(&m.stmt) == effect::Effect::Mutating,

            Operation::Alias(_)
            | Operation::Const(_)
            | Operation::Eval(_)
            | Operation::Filter(_)
            | Operation::FindPkByIndex(_)
            | Operation::GetByKey(_)
            | Operation::NestedMerge(_)
            | Operation::Project(_)
            | Operation::QueryPk(_)
            | Operation::Scan(_) => false,
        }
    }
}

impl From<Operation> for Node {
    fn from(value: Operation) -> Self {
        let deps = value.inputs();

        Node {
            op: value,
            deps,
            var: Cell::new(None),
            num_uses: Cell::new(0),
            guard: None,
            visited: Cell::new(false),
        }
    }
}
