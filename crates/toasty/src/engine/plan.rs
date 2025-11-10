mod action;
pub(crate) use action::Action;

mod delete_by_key;
pub(crate) use delete_by_key::DeleteByKey;

mod exec_statement2;
pub(crate) use exec_statement2::{ExecStatement2, ExecStatementOutput};

mod filter;
pub(crate) use filter::Filter;

mod find_pk_by_index;
pub(crate) use find_pk_by_index::FindPkByIndex2;

mod get_by_key;
pub(crate) use get_by_key::GetByKey2;

mod nested_merge;
pub(crate) use nested_merge::{MergeQualification, NestedChild, NestedLevel, NestedMerge};

mod output;
pub(crate) use output::Output2;

mod pipeline;
pub(crate) use pipeline::Pipeline;

mod project;
pub(crate) use project::Project;

mod query_pk;
pub(crate) use query_pk::QueryPk2;

mod rmw;
pub(crate) use rmw::ReadModifyWrite2;

mod set_var;
pub(crate) use set_var::{SetVar2, VarId};

mod update_by_key;
pub(crate) use update_by_key::UpdateByKey;

use crate::engine::exec;

#[derive(Debug)]
pub(crate) struct Plan {
    /// Arguments seeding the plan
    pub(crate) vars: exec::VarStore,

    /// Pipeline of steps
    pub(crate) pipeline: Pipeline,
}
