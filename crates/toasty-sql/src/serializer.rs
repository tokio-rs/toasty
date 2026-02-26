#[macro_use]
mod fmt;
use std::borrow::Cow;

use fmt::ToSql;

mod column;
use column::ColumnAlias;

mod cte;

mod delim;
use delim::{Comma, Delimited, Period};

mod flavor;
use flavor::Flavor;

mod ident;
use ident::Ident;

mod params;
pub use params::{Params, Placeholder, TypedValue};

// Fragment serializers
mod column_def;
mod expr;
mod name;
mod statement;
mod ty;
mod value;

use crate::stmt::Statement;

use toasty_core::{
    driver::operation::Transaction,
    schema::db::{self, Index, Table},
};

/// Context information when serializing VALUES in an INSERT statement
#[derive(Debug, Clone)]
pub struct InsertContext {
    pub table_id: db::TableId,
    pub columns: Vec<db::ColumnId>,
}

/// Serialize a statement to a SQL string
#[derive(Debug)]
pub struct Serializer<'a> {
    /// Schema against which the statement is to be serialized
    schema: &'a db::Schema,

    /// The database flavor handles the differences between SQL dialects and
    /// supported features.
    flavor: Flavor,
}

struct Formatter<'a, T> {
    /// Handle to the serializer
    serializer: &'a Serializer<'a>,

    /// Where to write the serialized SQL
    dst: &'a mut String,

    /// Where to store parameters
    params: &'a mut T,

    /// Current query depth. This is used to determine the nesting level when
    /// generating names
    depth: usize,

    /// True when table names should be aliased.
    alias: bool,

    /// Context when serializing VALUES in an INSERT statement
    insert_context: Option<InsertContext>,
}

pub type ExprContext<'a> = toasty_core::stmt::ExprContext<'a, db::Schema>;

impl<'a> Serializer<'a> {
    pub fn serialize(&self, stmt: &Statement, params: &mut impl Params) -> String {
        let mut ret = String::new();

        let mut fmt = Formatter {
            serializer: self,
            dst: &mut ret,
            params,
            depth: 0,
            alias: false,
            insert_context: None,
        };

        let cx = ExprContext::new(self.schema);

        stmt.to_sql(&cx, &mut fmt);

        ret.push(';');
        ret
    }

    /// Serialize a transaction control operation to a SQL string.
    ///
    /// The generated SQL is flavor-specific (e.g., MySQL uses `START TRANSACTION`
    /// while other databases use `BEGIN`). Savepoints are named `sp_{id}`.
    pub fn serialize_transaction(&self, op: &Transaction) -> Cow<'static, str> {
        match op {
            Transaction::Start { isolation } => match (&self.flavor, isolation) {
                (Flavor::Mysql, None) => "START TRANSACTION".into(),
                (Flavor::Mysql, Some(level)) => format!(
                    "SET TRANSACTION ISOLATION LEVEL {}; START TRANSACTION",
                    level.sql_name()
                )
                .into(),
                (_, None) => "BEGIN".into(),
                (_, Some(level)) => format!("BEGIN ISOLATION LEVEL {}", level.sql_name()).into(),
            },
            Transaction::Commit => "COMMIT".into(),
            Transaction::Rollback => "ROLLBACK".into(),
            Transaction::Savepoint(id) => format!("SAVEPOINT sp_{id}").into(),
            Transaction::ReleaseSavepoint(id) => format!("RELEASE SAVEPOINT sp_{id}").into(),
            Transaction::RollbackToSavepoint(id) => format!("ROLLBACK TO SAVEPOINT sp_{id}").into(),
        }
    }

    fn table(&self, id: impl Into<db::TableId>) -> &'a Table {
        self.schema.table(id.into())
    }

    fn index(&self, id: impl Into<db::IndexId>) -> &'a Index {
        self.schema.index(id.into())
    }

    fn table_name(&self, id: impl Into<db::TableId>) -> Ident<&str> {
        let table = self.schema.table(id.into());
        Ident(&table.name)
    }

    fn column_name(&self, id: impl Into<db::ColumnId>) -> Ident<&str> {
        let column = self.schema.column(id.into());
        Ident(&column.name)
    }
}
