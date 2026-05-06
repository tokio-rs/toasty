mod add_column;
pub use add_column::AddColumn;

mod alter_column;
pub use alter_column::{AlterColumn, AlterColumnChanges};

mod alter_table;
pub use alter_table::{AlterTable, AlterTableAction};

mod alter_type;
pub use alter_type::AlterType;

mod check;
pub use check::CheckConstraint;

mod column_def;
pub use column_def::ColumnDef;

mod copy_table;
pub use copy_table::CopyTable;

mod create_index;
pub use create_index::CreateIndex;

mod create_table;
pub use create_table::CreateTable;

mod create_type;
pub use create_type::CreateType;

mod drop_column;
pub use drop_column::DropColumn;

mod drop_index;
pub use drop_index::DropIndex;

mod drop_table;
pub use drop_table::DropTable;

mod ident;
pub use ident::Ident;

mod name;
pub use name::Name;

mod pragma;
pub use pragma::Pragma;

mod table_name;
pub use table_name::TableName;

pub use toasty_core::stmt::*;

/// A SQL statement, covering both DDL (schema changes) and DML (data manipulation).
#[derive(Debug, Clone)]
pub enum Statement {
    /// Add a column to an existing table.
    AddColumn(AddColumn),
    /// Alter properties of an existing column.
    AlterColumn(AlterColumn),
    /// Alter an existing table (e.g. rename).
    AlterTable(AlterTable),
    /// Alter a type (e.g. `ALTER TYPE ... ADD VALUE '...'`).
    AlterType(AlterType),
    /// Copy rows from one table to another.
    CopyTable(CopyTable),
    /// Create an index.
    CreateIndex(CreateIndex),
    /// Create a table.
    CreateTable(CreateTable),
    /// Create a type (e.g. `CREATE TYPE ... AS ENUM (...)`).
    CreateType(CreateType),
    /// Drop a column from an existing table.
    DropColumn(DropColumn),
    /// Drop a table.
    DropTable(DropTable),
    /// Drop an index.
    DropIndex(DropIndex),
    /// A SQLite PRAGMA statement.
    Pragma(Pragma),
    /// A DELETE statement.
    Delete(Delete),
    /// An INSERT statement.
    Insert(Insert),
    /// A SELECT query.
    Query(Query),
    /// An UPDATE statement.
    Update(Update),
}

impl Statement {
    /// Returns `true` if this is an [`Update`] statement.
    pub fn is_update(&self) -> bool {
        matches!(self, Self::Update(_))
    }

    /// Returns the number of returned elements within the statement (if one exists).
    pub fn returning_len(&self) -> Option<usize> {
        match self {
            Self::Delete(delete) => delete
                .returning
                .as_ref()
                .map(|ret| ret.as_project_unwrap().as_record_unwrap().len()),
            Self::Insert(insert) => insert
                .returning
                .as_ref()
                .map(|ret| ret.as_project_unwrap().as_record_unwrap().len()),
            Self::Query(query) => match &query.body {
                ExprSet::Select(select) => Some(
                    select
                        .returning
                        .as_project_unwrap()
                        .as_record_unwrap()
                        .len(),
                ),
                stmt => todo!("returning_len, stmt={stmt:#?}"),
            },
            Self::Update(update) => update
                .returning
                .as_ref()
                .map(|ret| ret.as_project_unwrap().as_record_unwrap().len()),
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
