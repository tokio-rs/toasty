use indexmap::IndexSet;

use crate::engine::effect;

use super::{
    Alias, Const, DeleteByKey, Eval, ExecStatement, Filter, FindPkByIndex, GetByKey, NestedMerge,
    Node, NodeId, QueryPk, ReadModifyWrite, Repeat, Scan, UpdateByKey, Upsert,
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

    /// Evaluate a function, once over whole input values or once per row of
    /// a base input
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

    /// Read-modify-write. The write only succeeds if the values read are not
    /// modified.
    ReadModifyWrite(Box<ReadModifyWrite>),

    /// Produce a constant value once per input row
    Repeat(Repeat),

    QueryPk(QueryPk),

    /// Full-table scan — emitted when no index covers the filter on a scan-capable driver.
    Scan(Scan),

    UpdateByKey(UpdateByKey),

    /// Atomically insert or update one record on a non-SQL database.
    Upsert(Box<Upsert>),
}

/// How an operation reads one of its inputs.
#[derive(Debug, Clone, Copy)]
pub(crate) enum InputRead {
    /// Read whenever the operation runs.
    Always,

    /// Read only while iterating the referenced node's rows; when that node
    /// returns no rows, the input is never read.
    PerRowOf(NodeId),
}

impl Operation {
    /// Returns the operation's variant name for logging.
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Operation::Alias(_) => "alias",
            Operation::Const(_) => "const",
            Operation::DeleteByKey(_) => "delete_by_key",
            Operation::Eval(_) => "eval",
            Operation::ExecStatement(_) => "exec_statement",
            Operation::Filter(_) => "filter",
            Operation::FindPkByIndex(_) => "find_pk_by_index",
            Operation::GetByKey(_) => "get_by_key",
            Operation::NestedMerge(_) => "nested_merge",
            Operation::ReadModifyWrite(_) => "read_modify_write",
            Operation::Repeat(_) => "repeat",
            Operation::QueryPk(_) => "query_pk",
            Operation::Scan(_) => "scan",
            Operation::UpdateByKey(_) => "update_by_key",
            Operation::Upsert(_) => "upsert",
        }
    }

    /// True for operations that issue a driver operation.
    ///
    /// Used to determine whether a plan needs to be wrapped in a transaction.
    /// Distinct from [`Operation::is_effectful`]: queries are db ops but not
    /// effectful. In-memory operations count zero.
    pub(crate) fn is_db_op(&self) -> bool {
        match self {
            Operation::DeleteByKey(_)
            | Operation::ExecStatement(_)
            | Operation::FindPkByIndex(_)
            | Operation::GetByKey(_)
            | Operation::QueryPk(_)
            | Operation::ReadModifyWrite(_)
            | Operation::Scan(_)
            | Operation::UpdateByKey(_)
            | Operation::Upsert(_) => true,

            Operation::Alias(_)
            | Operation::Const(_)
            | Operation::Eval(_)
            | Operation::Filter(_)
            | Operation::NestedMerge(_)
            | Operation::Repeat(_) => false,
        }
    }

    /// The operation's value edges — nodes whose outputs it reads — each
    /// paired with how the output is read.
    ///
    /// `Node.deps` is seeded from these nodes and may then be extended with
    /// ordering-only edges (e.g. "child INSERT before parent INSERT"). The
    /// ordering-only edges are derivable as `deps − inputs()`.
    pub(crate) fn input_reads(&self) -> Vec<(NodeId, InputRead)> {
        use InputRead::{Always, PerRowOf};

        match self {
            Operation::Alias(m) => vec![(m.input, Always)],
            Operation::Const(_m) => vec![],
            Operation::DeleteByKey(m) => vec![(m.input, Always)],
            Operation::Eval(m) => match m.base {
                Some(base) => [(base, Always)]
                    .into_iter()
                    .chain(m.attached.iter().map(|&a| (a, PerRowOf(base))))
                    .collect(),
                None => m.attached.iter().map(|&i| (i, Always)).collect(),
            },
            Operation::ExecStatement(m) => m.inputs.iter().map(|&i| (i, Always)).collect(),
            Operation::Filter(m) => vec![(m.input, Always)],
            Operation::FindPkByIndex(m) => m.inputs.iter().map(|&i| (i, Always)).collect(),
            Operation::GetByKey(m) => vec![(m.input, Always)],
            Operation::NestedMerge(m) => m.inputs.iter().map(|&i| (i, Always)).collect(),
            Operation::ReadModifyWrite(m) => m.inputs.iter().map(|&i| (i, Always)).collect(),
            // The input's cardinality is observed, so the read is
            // unconditional even though no row data is used.
            Operation::Repeat(m) => vec![(m.input, Always)],
            Operation::QueryPk(m) => m.input.into_iter().map(|i| (i, Always)).collect(),
            Operation::Scan(m) => m.input.into_iter().map(|i| (i, Always)).collect(),
            Operation::UpdateByKey(m) => vec![(m.input, Always)],
            Operation::Upsert(m) => m.inputs.iter().map(|&i| (i, Always)).collect(),
        }
    }

    /// The operation's value edges: nodes whose outputs it reads.
    pub(crate) fn inputs(&self) -> IndexSet<NodeId> {
        self.input_reads().into_iter().map(|(id, _)| id).collect()
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
            | Operation::QueryPk(_)
            | Operation::Repeat(_)
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
            guard: None,
        }
    }
}
