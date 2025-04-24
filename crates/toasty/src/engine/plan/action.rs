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
    ReadModifyWrite(ReadModifyWrite),

    /// Set a variable to a const
    SetVar(SetVar),

    /// Update a record by the primary key
    UpdateByKey(UpdateByKey),
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Associate(a) => a.fmt(f),
            Action::BatchWrite(a) => a.fmt(f),
            Action::DeleteByKey(a) => a.fmt(f),
            Action::ExecStatement(a) => a.fmt(f),
            Action::FindPkByIndex(a) => a.fmt(f),
            Action::GetByKey(a) => a.fmt(f),
            Action::Insert(a) => a.fmt(f),
            Action::QueryPk(a) => a.fmt(f),
            Action::ReadModifyWrite(a) => a.fmt(f),
            Action::SetVar(a) => a.fmt(f),
            Action::UpdateByKey(a) => a.fmt(f),
        }
    }
}
