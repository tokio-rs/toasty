use crate::engine::exec::{
    Alias, DeleteByKey, Eval, ExecStatement, Filter, FindPkByIndex, GetByKey, If, NestedMerge,
    QueryPk, ReadModifyWrite, Release, Repeat, Scan, SetVar, UpdateByKey, Upsert,
};

use std::fmt;

pub(crate) enum Action {
    /// Pass a variable's value through to another slot unchanged
    Alias(Alias),

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

    /// Conditionally execute a block of pure actions
    If(If),

    /// Combines parent and child data into nested structures.
    ///
    /// Loads all batch data upfront, then recursively processes each row by filtering
    /// and merging child data at all nesting levels, finally projecting each row with
    /// its nested children into the final result.
    NestedMerge(NestedMerge),

    /// Query records by primary key
    QueryPk(QueryPk),

    /// Decrement a variable's use count without observing its value
    Release(Release),

    /// Produce a constant value once per input row
    Repeat(Repeat),

    /// Perform a full-table scan
    Scan(Scan),

    /// Perform an atomic operation in multiple steps
    ReadModifyWrite(Box<ReadModifyWrite>),

    /// Set a variable to a const
    SetVar(SetVar),

    /// Update a record by the primary key
    UpdateByKey(UpdateByKey),

    /// Atomically insert or update one record on a non-SQL database.
    Upsert(Upsert),
}

impl Action {
    /// Returns the action variant name for logging.
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Action::Alias(_) => "alias",
            Action::DeleteByKey(_) => "delete_by_key",
            Action::Eval(_) => "eval",
            Action::ExecStatement(_) => "exec_statement",
            Action::Filter(_) => "filter",
            Action::FindPkByIndex(_) => "find_pk_by_index",
            Action::GetByKey(_) => "get_by_key",
            Action::If(_) => "if",
            Action::NestedMerge(_) => "nested_merge",
            Action::QueryPk(_) => "query_pk",
            Action::ReadModifyWrite(_) => "read_modify_write",
            Action::Release(_) => "release",
            Action::Repeat(_) => "repeat",
            Action::Scan(_) => "scan",
            Action::SetVar(_) => "set_var",
            Action::UpdateByKey(_) => "update_by_key",
            Action::Upsert(_) => "upsert",
        }
    }

    /// Returns the number of database operations this action issues, counting
    /// into `If` arms.
    ///
    /// Used to determine whether a plan needs to be wrapped in a transaction.
    /// The count is static: a skipped `If` arm can leave a transaction
    /// wrapping a single executed operation, which is harmless. In-memory
    /// actions (Alias, Filter, Repeat, NestedMerge, SetVar, Eval, Release)
    /// count zero.
    pub(crate) fn db_op_count(&self) -> usize {
        match self {
            Action::DeleteByKey(_)
            | Action::ExecStatement(_)
            | Action::FindPkByIndex(_)
            | Action::GetByKey(_)
            | Action::QueryPk(_)
            | Action::ReadModifyWrite(_)
            | Action::Scan(_)
            | Action::UpdateByKey(_)
            | Action::Upsert(_) => 1,

            Action::If(action) => action.then.iter().map(Action::db_op_count).sum(),

            Action::Alias(_)
            | Action::Eval(_)
            | Action::Filter(_)
            | Action::NestedMerge(_)
            | Action::Release(_)
            | Action::Repeat(_)
            | Action::SetVar(_) => 0,
        }
    }
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alias(a) => a.fmt(f),
            Self::DeleteByKey(a) => a.fmt(f),
            Self::Eval(a) => a.fmt(f),
            Self::ExecStatement(a) => a.fmt(f),
            Self::Filter(a) => a.fmt(f),
            Self::FindPkByIndex(a) => a.fmt(f),
            Self::GetByKey(a) => a.fmt(f),
            Self::If(a) => a.fmt(f),
            Self::NestedMerge(a) => a.fmt(f),
            Self::QueryPk(a) => a.fmt(f),
            Self::ReadModifyWrite(a) => a.fmt(f),
            Self::Release(a) => a.fmt(f),
            Self::Repeat(a) => a.fmt(f),
            Self::Scan(a) => a.fmt(f),
            Self::SetVar(a) => a.fmt(f),
            Self::UpdateByKey(a) => a.fmt(f),
            Self::Upsert(a) => a.fmt(f),
        }
    }
}
