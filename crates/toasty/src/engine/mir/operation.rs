use std::cell::Cell;

use indexmap::{indexset, IndexSet};

use crate::engine::mir::Eval;

use super::{
    Const, DeleteByKey, ExecStatement, Filter, FindPkByIndex, GetByKey, NestedMerge, Node, Project,
    QueryPk, ReadModifyWrite, UpdateByKey,
};

/// A step in the query execution plan.
///
/// Operations represent units of work: loading data from the database,
/// filtering results, transforming records, or combining nested data.
#[derive(Debug)]
pub(crate) enum Operation {
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

    UpdateByKey(UpdateByKey),
}

impl From<Operation> for Node {
    fn from(value: Operation) -> Self {
        let deps = match &value {
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
