use super::*;

pub(crate) enum Action {
    /// Associate a preloaded relation with the owner
    Associate(Associate),

    /// Perform a batch write
    BatchWrite(BatchWrite),

    /// Delete a record by the primary key
    DeleteByKey(DeleteByKey),

    /// Execute a statement
    ExecStatement(ExecStatement),

    FindPkByIndex(FindPkByIndex),

    /// Execute `Operation::GetByKey` using key input
    GetByKey(GetByKey),

    /// Insert a record
    Insert(Insert),

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
            Self::FindPkByIndex(a) => a.fmt(f),
            Self::GetByKey(a) => a.fmt(f),
            Self::Insert(a) => a.fmt(f),
            Self::QueryPk(a) => a.fmt(f),
            Self::ReadModifyWrite(a) => a.fmt(f),
            Self::SetVar(a) => a.fmt(f),
            Self::UpdateByKey(a) => a.fmt(f),
        }
    }
}
