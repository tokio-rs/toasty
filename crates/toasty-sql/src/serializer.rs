#[macro_use]
mod fmt;
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
    driver::operation::{IsolationLevel, Transaction},
    schema::db::{self, Index, Table},
};

/// Context information when serializing VALUES in an INSERT statement.
#[derive(Debug, Clone)]
pub struct InsertContext {
    /// The table being inserted into.
    pub table_id: db::TableId,
    /// Columns receiving values, in order.
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

    /// When `true`, values are emitted as bind-parameter placeholders and
    /// pushed into `params`. When `false`, values are inlined as SQL literals.
    /// DDL statements (CREATE TABLE, etc.) set this to `false` because they
    /// do not support bind parameters.
    bind_params: bool,

    /// Context when serializing VALUES in an INSERT statement
    insert_context: Option<InsertContext>,
}

/// Expression context bound to a database-level schema.
pub type ExprContext<'a> = toasty_core::stmt::ExprContext<'a, db::Schema>;

/// A no-op [`Params`] implementation for DDL serialization where bind
/// parameters are not used.
struct NoParams;

impl Params for NoParams {
    fn push(
        &mut self,
        _: &toasty_core::stmt::Value,
        _: Option<&toasty_core::stmt::Type>,
    ) -> params::Placeholder {
        unreachable!("DDL serialization should not produce bind parameters")
    }
}

impl<'a> Serializer<'a> {
    /// Serializes a [`Statement`] to a SQL string with all values inlined as
    /// literals (no bind parameters). Appends a trailing semicolon.
    ///
    /// Use this for DDL statements (`CREATE TABLE`, `CREATE TYPE`, etc.) where
    /// bind parameters are not supported.
    pub fn serialize(&self, stmt: &Statement) -> String {
        let mut ret = String::new();

        let mut fmt = Formatter {
            serializer: self,
            dst: &mut ret,
            params: &mut NoParams,
            depth: 0,
            alias: false,
            bind_params: false,
            insert_context: None,
        };

        let cx = ExprContext::new(self.schema);

        stmt.to_sql(&cx, &mut fmt);

        ret.push(';');
        ret
    }

    /// Serializes a [`Statement`] to a SQL string, appending a trailing semicolon.
    ///
    /// Parameter placeholders are written in the dialect's native format
    /// (`$1` for PostgreSQL, `?1` for SQLite, `?` for MySQL) and the
    /// corresponding values are pushed into `params`.
    pub fn serialize_with_params(&self, stmt: &Statement, params: &mut impl Params) -> String {
        let mut ret = String::new();

        let mut fmt = Formatter {
            serializer: self,
            dst: &mut ret,
            params,
            depth: 0,
            alias: false,
            bind_params: true,
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
    pub fn serialize_transaction(&self, op: &Transaction) -> String {
        let mut ret = String::new();

        let mut f = Formatter {
            serializer: self,
            dst: &mut ret,
            params: &mut Vec::<TypedValue>::new(),
            depth: 0,
            alias: false,
            bind_params: false,
            insert_context: None,
        };

        let cx = ExprContext::new(self.schema);

        match op {
            Transaction::Start {
                isolation,
                read_only,
            } => fmt!(
                &cx,
                &mut f,
                self.serialize_transaction_start(*isolation, *read_only)
            ),
            Transaction::Commit => fmt!(&cx, &mut f, "COMMIT"),
            Transaction::Rollback => fmt!(&cx, &mut f, "ROLLBACK"),
            Transaction::Savepoint(name) => {
                fmt!(&cx, &mut f, "SAVEPOINT " Ident(name))
            }
            Transaction::ReleaseSavepoint(name) => {
                fmt!(&cx, &mut f, "RELEASE SAVEPOINT " Ident(name))
            }
            Transaction::RollbackToSavepoint(name) => {
                fmt!(&cx, &mut f, "ROLLBACK TO SAVEPOINT " Ident(name))
            }
        };

        ret.push(';');
        ret
    }

    fn serialize_transaction_start(
        &self,
        isolation: Option<IsolationLevel>,
        read_only: bool,
    ) -> String {
        fn isolation_level_str(level: IsolationLevel) -> &'static str {
            match level {
                IsolationLevel::ReadUncommitted => "READ UNCOMMITTED",
                IsolationLevel::ReadCommitted => "READ COMMITTED",
                IsolationLevel::RepeatableRead => "REPEATABLE READ",
                IsolationLevel::Serializable => "SERIALIZABLE",
            }
        }

        match self.flavor {
            Flavor::Mysql => {
                let mut sql = String::new();
                if let Some(level) = isolation {
                    sql.push_str("SET TRANSACTION ISOLATION LEVEL ");
                    sql.push_str(isolation_level_str(level));
                    sql.push_str("; ");
                }
                sql.push_str("START TRANSACTION");
                if read_only {
                    sql.push_str(" READ ONLY");
                }
                sql
            }
            Flavor::Postgresql => {
                let mut sql = String::from("BEGIN");
                if let Some(level) = isolation {
                    sql.push_str(" ISOLATION LEVEL ");
                    sql.push_str(isolation_level_str(level));
                }
                if read_only {
                    sql.push_str(" READ ONLY");
                }
                sql
            }
            Flavor::Sqlite => {
                // SQLite doesn't support per-transaction isolation levels or read-only mode
                "BEGIN".to_string()
            }
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
