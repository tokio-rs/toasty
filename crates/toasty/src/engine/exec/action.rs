use crate::engine::exec::{
    DeleteByKey, Eval, ExecStatement, Filter, FindPkByIndex, GetByKey, NestedMerge, Project,
    QueryPk, ReadModifyWrite, SetVar, UpdateByKey,
};

use std::fmt;

pub(crate) enum Action {
    /// Delete a record by the primary key
    DeleteByKey(DeleteByKey),

    /// Evaluate a function in memory
    Eval(Eval),

    /// Execute a statement
    ExecStatement(Box<ExecStatement>),

    /// Filter a value stream
    Filter(Filter),

    FindPkByIndex(FindPkByIndex),

    /// Execute `Operation::GetByKey` using key input
    GetByKey(GetByKey),

    /// Combines parent and child data into nested structures.
    ///
    /// Loads all batch data upfront, then recursively processes each row by filtering
    /// and merging child data at all nesting levels, finally projecting each row with
    /// its nested children into the final result.
    NestedMerge(NestedMerge),

    /// Take the contents of a variable and project it one or more times to a
    /// specified variable.
    Project(Project),

    /// Query records by primary key
    QueryPk(QueryPk),

    /// Perform an atomic operation in multiple steps
    ReadModifyWrite(Box<ReadModifyWrite>),

    /// Set a variable to a const
    SetVar(SetVar),

    /// Update a record by the primary key
    UpdateByKey(UpdateByKey),
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeleteByKey(a) => a.fmt(f),
            Self::Eval(a) => a.fmt(f),
            Self::ExecStatement(a) => a.fmt(f),
            Self::Filter(a) => a.fmt(f),
            Self::FindPkByIndex(a) => a.fmt(f),
            Self::GetByKey(a) => a.fmt(f),
            Self::NestedMerge(a) => a.fmt(f),
            Self::QueryPk(a) => a.fmt(f),
            Self::ReadModifyWrite(a) => a.fmt(f),
            Self::Project(a) => a.fmt(f),
            Self::SetVar(a) => a.fmt(f),
            Self::UpdateByKey(a) => a.fmt(f),
        }
    }
}
