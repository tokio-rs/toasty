mod action;
pub(crate) use action::Action;

mod associate;
pub(crate) use associate::Associate;

mod batch_write;
pub(crate) use batch_write::{BatchWrite, WriteAction};

mod delete_by_key;
pub(crate) use delete_by_key::DeleteByKey;

mod exec_statement;
pub(crate) use exec_statement::ExecStatement;

mod find_pk_by_index;
pub(crate) use find_pk_by_index::FindPkByIndex;

mod get_by_key;
pub(crate) use get_by_key::GetByKey;

mod input;
pub(crate) use input::{Input, InputSource};

mod insert;
pub(crate) use insert::Insert;

mod output;
pub(crate) use output::Output;

mod pipeline;
pub(crate) use pipeline::Pipeline;

mod query_pk;
pub(crate) use query_pk::QueryPk;

mod rmw;
pub(crate) use rmw::ReadModifyWrite;

mod set_var;
pub(crate) use set_var::{SetVar, VarId};

mod update_by_key;
pub(crate) use update_by_key::UpdateByKey;

use super::*;
use std::fmt;

#[derive(Debug)]
pub(crate) struct Plan {
    /// Arguments seeding the plan
    pub(crate) vars: exec::VarStore,

    /// Pipeline of steps
    pub(crate) pipeline: Pipeline,
}
