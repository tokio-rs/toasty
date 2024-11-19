use super::*;

pub(crate) enum Action {
    /// Associate a preloaded relation with the owner
    Associate(Associate),

    /// Perform a batch write
    BatchWrite(BatchWrite),

    DeleteByKey(DeleteByKey),

    FindPkByIndex(FindPkByIndex),

    /// Execute `Operation::GetByKey` using key input
    GetByKey(GetByKey),

    Insert(Insert),

    QueryPk(QueryPk),

    /// Update a record by the primary key
    UpdateByKey(UpdateByKey),

    /// Set a variable to a const
    SetVar(SetVar),

    /// Issue a SQL query
    QuerySql(QuerySql),
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Associate(a) => a.fmt(f),
            Action::BatchWrite(a) => a.fmt(f),
            Action::DeleteByKey(a) => a.fmt(f),
            Action::FindPkByIndex(a) => a.fmt(f),
            Action::GetByKey(a) => a.fmt(f),
            Action::Insert(a) => a.fmt(f),
            Action::QueryPk(a) => a.fmt(f),
            Action::QuerySql(a) => a.fmt(f),
            Action::UpdateByKey(a) => a.fmt(f),
            Action::SetVar(a) => a.fmt(f),
        }
    }
}
