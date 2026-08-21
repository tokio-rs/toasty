use indexmap::IndexSet;

use super::{NodeId, Operation};

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
            Operation::Eval(m) => match m.row_input {
                Some(row_input) => [(row_input, Always)]
                    .into_iter()
                    .chain(m.inputs.iter().map(|&input| (input, PerRowOf(row_input))))
                    .collect(),
                None => m.inputs.iter().map(|&input| (input, Always)).collect(),
            },
            Operation::ExecStatement(m) => m.inputs.iter().map(|&i| (i, Always)).collect(),
            Operation::Filter(m) => [(m.input, Always)]
                .into_iter()
                .chain(m.args.iter().map(|&a| (a, Always)))
                .collect(),
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

    /// The variable loads the operation's execution performs, with
    /// multiplicity — currently one per declared input. `num_uses` refcounts
    /// are the sum of these loads across consumers, so each operation must
    /// load every listed input exactly once — or release it on any path that
    /// declines the load. (Guards peek without loading.)
    pub(crate) fn input_loads(&self) -> impl Iterator<Item = NodeId> + use<> {
        // ReadModifyWrite declares `inputs` but its exec action asserts them
        // empty; count them anyway so a future non-empty RMW input must load
        // them rather than silently violating the counting.
        self.inputs().into_iter()
    }
}
