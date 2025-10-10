use crate::engine::plan::{
    exec_statement2::ExecStatement2, project::Project, FindPkByIndex2, GetByKey2,
};

use super::{
    Associate, BatchWrite, DeleteByKey, ExecStatement, FindPkByIndex, GetByKey, Insert,
    NestedMerge, QueryPk, ReadModifyWrite, SetVar, UpdateByKey,
};
use std::fmt;

pub(crate) enum Action {
    /// Associate a preloaded relation with the owner
    Associate(Associate),

    /// Perform a batch write
    BatchWrite(BatchWrite),

    /// Delete a record by the primary key
    DeleteByKey(DeleteByKey),

    /// Execute a statement
    ExecStatement(ExecStatement),

    /// Execute a statement
    ExecStatement2(ExecStatement2),

    FindPkByIndex(FindPkByIndex),
    FindPkByIndex2(FindPkByIndex2),

    /// Execute `Operation::GetByKey` using key input
    GetByKey(GetByKey),
    GetByKey2(GetByKey2),

    /// Insert a record
    Insert(Insert),

    /// Nested merge operation - combines parent and child materializations
    /// Handles the ENTIRE nesting hierarchy, not just one level
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
            Self::Associate(a) => a.fmt(f),
            Self::BatchWrite(a) => a.fmt(f),
            Self::DeleteByKey(a) => a.fmt(f),
            Self::ExecStatement(a) => a.fmt(f),
            Self::ExecStatement2(a) => a.fmt(f),
            Self::FindPkByIndex(a) => a.fmt(f),
            Self::FindPkByIndex2(a) => a.fmt(f),
            Self::GetByKey(a) => a.fmt(f),
            Self::GetByKey2(a) => a.fmt(f),
            Self::Insert(a) => a.fmt(f),
            Self::NestedMerge(a) => a.fmt(f),
            Self::QueryPk(a) => a.fmt(f),
            Self::ReadModifyWrite(a) => a.fmt(f),
            Self::Project(a) => a.fmt(f),
            Self::SetVar(a) => a.fmt(f),
            Self::UpdateByKey(a) => a.fmt(f),
        }
    }
}
