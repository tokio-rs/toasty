mod associate;
pub(crate) use associate::Associate;

mod batch_write;
pub(crate) use batch_write::{BatchWrite, WriteAction};

mod delete_by_key;
pub(crate) use delete_by_key::DeleteByKey;

mod find_pk_by_index;
pub(crate) use find_pk_by_index::FindPkByIndex;

mod get_by_key;
pub(crate) use get_by_key::GetByKey;

mod input;
pub(crate) use input::{Input, InputSource};

mod insert;
pub(crate) use insert::{Insert, InsertOutput};

mod query_pk;
pub(crate) use query_pk::QueryPk;

mod query_sql;
pub(crate) use query_sql::{QuerySql, QuerySqlOutput};

mod set_var;
pub(crate) use set_var::SetVar;

mod update_by_key;
pub(crate) use update_by_key::UpdateByKey;

use super::*;
use std::fmt;

#[derive(Debug)]
pub(crate) struct Plan<'stmt> {
    /// Arguments seeding the plan
    pub(crate) vars: exec::VarStore,

    /// Pipeline of steps
    pub(crate) pipeline: Pipeline<'stmt>,
}

#[derive(Debug)]
pub(crate) struct Pipeline<'stmt> {
    /// Steps in the pipeline
    pub(crate) actions: Vec<Action<'stmt>>,

    /// Which record stream slot does the pipeline return
    ///
    /// When `None`, nothing is returned
    pub(crate) returning: Option<VarId>,
}

pub(crate) enum Action<'stmt> {
    /// Associate a preloaded relation with the owner
    Associate(Associate),

    /// Perform a batch write
    BatchWrite(BatchWrite<'stmt>),

    DeleteByKey(DeleteByKey<'stmt>),

    /// Execute `Operation::FindPkByIndex`
    FindPkByIndex(FindPkByIndex<'stmt>),

    /// Execute `Operation::GetByKey` using key input
    GetByKey(GetByKey<'stmt>),

    Insert(Insert<'stmt>),

    QueryPk(QueryPk<'stmt>),

    /// Update a record by the primary key
    UpdateByKey(UpdateByKey<'stmt>),

    /// Set a variable to a const
    SetVar(SetVar),

    /// Issue a SQL query
    QuerySql(QuerySql<'stmt>),
}

/// Identifies a pipeline variable slot
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct VarId(pub(crate) usize);

impl fmt::Debug for Action<'_> {
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
