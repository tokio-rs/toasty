mod action;
pub(crate) use action::Action;

mod delete_by_key;
pub(crate) use delete_by_key::DeleteByKey;

mod exec_statement;
pub(crate) use exec_statement::{ExecStatement, ExecStatementOutput};

mod filter;
pub(crate) use filter::Filter;

mod find_pk_by_index;
pub(crate) use find_pk_by_index::FindPkByIndex;

mod get_by_key;
pub(crate) use get_by_key::GetByKey;

mod nested_merge;
pub(crate) use nested_merge::{MergeQualification, NestedChild, NestedLevel, NestedMerge};

mod output;
pub(crate) use output::Output;

mod project;
pub(crate) use project::Project;

mod query_pk;
pub(crate) use query_pk::QueryPk;

mod rmw;
pub(crate) use rmw::ReadModifyWrite;

mod set_var;
pub(crate) use set_var::{SetVar, VarId};

mod update_by_key;
pub(crate) use update_by_key::UpdateByKey;

use crate::engine::exec;

#[derive(Debug)]
pub(crate) struct Plan {
    /// Arguments seeding the plan
    pub(crate) vars: exec::VarStore,

    /// Steps in the pipeline
    pub(crate) actions: Vec<Action>,

    /// Which record stream slot does the pipeline return
    ///
    /// When `None`, nothing is returned
    pub(crate) returning: Option<VarId>,
}
