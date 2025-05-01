mod column_def;
pub use column_def::ColumnDef;

mod create_index;
pub use create_index::CreateIndex;

mod create_table;
pub use create_table::CreateTable;

mod drop_table;
pub use drop_table::DropTable;

mod name;
pub use name::Name;

pub use toasty_core::stmt::*;

#[derive(Debug, Clone)]
pub enum Statement {
    CreateIndex(CreateIndex),
    CreateTable(CreateTable),
    DropTable(DropTable),
    Delete(Delete),
    Insert(Insert),
    Query(Query),
    Update(Update),
}

impl Statement {
    pub fn is_update(&self) -> bool {
        matches!(self, Self::Update(_))
    }

    /// Returns the number of returned elements within the statement (if one exists).
    pub fn returning_len(&self) -> Option<usize> {
        match self {
            Self::Delete(delete) => delete
                .returning
                .as_ref()
                .map(|ret| ret.as_expr().as_record().len()),
            Self::Insert(insert) => insert
                .returning
                .as_ref()
                .map(|ret| ret.as_expr().as_record().len()),
            Self::Query(query) => match &*query.body {
                ExprSet::Select(select) => Some(select.returning.as_expr().as_record().len()),
                stmt => todo!("returning_len, stmt={stmt:#?}"),
            },
            Self::Update(update) => update
                .returning
                .as_ref()
                .map(|ret| ret.as_expr().as_record().len()),
            _ => None,
        }
    }
}

impl From<toasty_core::stmt::Statement> for Statement {
    fn from(value: toasty_core::stmt::Statement) -> Self {
        match value {
            toasty_core::stmt::Statement::Delete(stmt) => Self::Delete(stmt),
            toasty_core::stmt::Statement::Insert(stmt) => Self::Insert(stmt),
            toasty_core::stmt::Statement::Query(stmt) => Self::Query(stmt),
            toasty_core::stmt::Statement::Update(stmt) => Self::Update(stmt),
        }
    }
}
