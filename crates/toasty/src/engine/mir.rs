mod r#const;
pub(crate) use r#const::Const;

mod delete_by_key;
pub(crate) use delete_by_key::DeleteByKey;

mod exec_statement;
pub(crate) use exec_statement::ExecStatement;

mod eval;
pub(crate) use eval::Eval;

mod filter;
pub(crate) use filter::Filter;

mod find_pk_by_index;
pub(crate) use find_pk_by_index::FindPkByIndex;

mod get_by_key;
pub(crate) use get_by_key::GetByKey;

mod guard;
pub(crate) use guard::Guard;

mod logical_plan;
pub(crate) use logical_plan::LogicalPlan;

mod nested_merge;
pub(crate) use nested_merge::NestedMerge;

mod node;
pub(crate) use node::Node;

mod operation;
pub(crate) use operation::Operation;

mod project;
pub(crate) use project::Project;

mod query_pk;
pub(crate) use query_pk::QueryPk;

mod read_modify_write;
pub(crate) use read_modify_write::ReadModifyWrite;

mod scan;
pub(crate) use scan::Scan;

mod store;
pub(crate) use store::{NodeId, Store};

mod update_by_key;
pub(crate) use update_by_key::UpdateByKey;

mod upsert;
pub(crate) use upsert::Upsert;

use toasty_core::stmt;

/// Extracts the per-row column types from a node's return type. A node
/// returning rows has type `List<Record<...>>` — the record field types tell
/// the driver how to decode each row. `Unit` means the node returns only a row
/// count, so there are no column types.
pub(crate) fn row_field_types(ty: &stmt::Type) -> Option<Vec<stmt::Type>> {
    match ty {
        stmt::Type::List(rows) => match &**rows {
            stmt::Type::Record(fields) => Some(fields.clone()),
            _ => todo!("row_field_types: ty={ty:#?}"),
        },
        stmt::Type::Unit => None,
        _ => todo!("row_field_types: ty={ty:#?}"),
    }
}
