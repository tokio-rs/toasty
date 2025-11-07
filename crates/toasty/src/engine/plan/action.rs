use crate::engine::plan::{
    DeleteByKey, ExecStatement2, Filter, FindPkByIndex2, GetByKey2, NestedMerge, Project, QueryPk2,
    ReadModifyWrite2, SetVar2, UpdateByKey,
};

use std::fmt;

pub(crate) enum Action {
    /// Delete a record by the primary key
    DeleteByKey(DeleteByKey),

    /// Execute a statement
    ExecStatement2(ExecStatement2),

    /// Filter a value stream
    Filter(Filter),

    FindPkByIndex2(FindPkByIndex2),

    /// Execute `Operation::GetByKey` using key input
    GetByKey2(GetByKey2),

    /// Nested merge operation - combines parent and child materializations
    /// Handles the ENTIRE nesting hierarchy, not just one level
    NestedMerge(NestedMerge),

    /// Take the contents of a variable and project it one or more times to a
    /// specified variable.
    Project(Project),

    /// Query records by primary key
    QueryPk2(QueryPk2),

    /// Perform an atomic operation in multiple steps
    ReadModifyWrite2(Box<ReadModifyWrite2>),

    /// Set a variable to a const
    SetVar2(SetVar2),

    /// Update a record by the primary key
    UpdateByKey(UpdateByKey),
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeleteByKey(a) => a.fmt(f),
            Self::ExecStatement2(a) => a.fmt(f),
            Self::Filter(a) => a.fmt(f),
            Self::FindPkByIndex2(a) => a.fmt(f),
            Self::GetByKey2(a) => a.fmt(f),
            Self::NestedMerge(a) => a.fmt(f),
            Self::QueryPk2(a) => a.fmt(f),
            Self::ReadModifyWrite2(a) => a.fmt(f),
            Self::Project(a) => a.fmt(f),
            Self::SetVar2(a) => a.fmt(f),
            Self::UpdateByKey(a) => a.fmt(f),
        }
    }
}
