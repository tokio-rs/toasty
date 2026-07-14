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
pub use params::Placeholder;

// Fragment serializers
mod column_def;
mod expr;
mod name;
mod statement;
mod ty;
mod value;

use crate::stmt::Statement;

use toasty_core::{
    driver::operation::{IsolationLevel, Transaction, TransactionMode},
    schema::db::{self, Index, Table},
    stmt::IntoExprTarget,
};

/// Serialize a statement to a SQL string
#[derive(Debug)]
pub struct Serializer<'a> {
    /// Schema against which the statement is to be serialized
    schema: &'a db::Schema,

    /// The database flavor handles the differences between SQL dialects and
    /// supported features.
    flavor: Flavor,

    /// SQL emitted for [`TransactionMode::Default`] under the SQLite flavor.
    /// Constructors that don't override this leave it at `"BEGIN"`, which is
    /// SQLite's natural default (DEFERRED). A driver that wants `Default` to
    /// mean something engine-specific — Turso under `concurrent_writes()`
    /// uses `"BEGIN CONCURRENT"` — sets this through
    /// [`Self::sqlite_with_default_begin`]. The non-`Default` variants
    /// (`Deferred`, `Immediate`, `Exclusive`) always emit fixed SQL.
    sqlite_default_begin: &'static str,
}

struct Formatter<'a> {
    /// Handle to the serializer
    serializer: &'a Serializer<'a>,

    /// Expression-resolution context for the current scope. Re-scoped (via
    /// [`Formatter::scope`]) each time serialization descends into a new
    /// query level, so it travels with the formatter rather than as a
    /// separate argument.
    cx: ExprContext<'a>,

    /// Where to write the serialized SQL
    dst: &'a mut String,

    /// Current query depth. This is used to determine the nesting level when
    /// generating names
    depth: usize,

    /// True when table names should be aliased.
    alias: bool,

    /// True when inside an INSERT statement. Used by MySQL to decide whether
    /// VALUES rows need the ROW() wrapper (required in subqueries but not in
    /// INSERT).
    in_insert: bool,

    /// Collects `Expr::Arg(n)` positions in the order they appear in the SQL.
    /// Used by MySQL (which uses positional `?` without indices) to reorder
    /// the params vec to match placeholder occurrence order. Borrowed so a
    /// scoped child formatter writes through to the root's vec.
    arg_positions: &'a mut Vec<usize>,
}

impl<'a> Formatter<'a> {
    /// Descend into a new expression scope, returning a child formatter that
    /// shares this one's output sink and arg collector (so writes flow back
    /// to the root) but resolves references against `target`.
    ///
    /// The child borrows `self`, so the parent scope stays live on the stack
    /// for the child's lifetime — that is what keeps the `ExprContext` parent
    /// chain valid for nested-reference resolution.
    fn scope<'c>(&'c mut self, target: impl IntoExprTarget<'c, db::Schema>) -> Formatter<'c> {
        Formatter {
            serializer: self.serializer,
            cx: self.cx.scope(target),
            dst: &mut *self.dst,
            depth: self.depth,
            alias: self.alias,
            in_insert: self.in_insert,
            arg_positions: &mut *self.arg_positions,
        }
    }
}

/// Expression context bound to a database-level schema.
pub type ExprContext<'a> = toasty_core::stmt::ExprContext<'a, db::Schema>;

impl<'a> Serializer<'a> {
    /// Serializes a [`Statement`] to a SQL string with all values inlined as
    /// literals (no bind parameters). Appends a trailing semicolon.
    ///
    /// Use this for DDL statements (`CREATE TABLE`, `CREATE TYPE`, etc.) where
    /// bind parameters are not supported. DML statements should already have
    /// their parameters extracted (as `Expr::Arg` placeholders) before reaching
    /// the serializer.
    pub fn serialize(&self, stmt: &Statement) -> String {
        self.serialize_with_arg_order(stmt).0
    }

    /// Serializes a [`Statement`] and returns both the SQL string and the order
    /// in which `Expr::Arg(n)` placeholders appear in the SQL.
    ///
    /// The arg order is needed by MySQL which uses positional `?` without
    /// indices — the caller must reorder its params vec to match the occurrence
    /// order. PostgreSQL and SQLite use indexed placeholders (`$1`, `?1`) so
    /// they can ignore the arg order.
    pub fn serialize_with_arg_order(&self, stmt: &Statement) -> (String, Vec<usize>) {
        let mut ret = String::new();
        let mut arg_positions = Vec::new();

        {
            let mut fmt = Formatter {
                serializer: self,
                cx: ExprContext::new(self.schema),
                dst: &mut ret,
                depth: 0,
                alias: false,
                in_insert: false,
                arg_positions: &mut arg_positions,
            };

            stmt.to_sql(&mut fmt);
        }

        ret.push(';');
        (ret, arg_positions)
    }

    /// Serialize a transaction control operation to a SQL string.
    ///
    /// The generated SQL is flavor-specific (e.g., MySQL uses `START TRANSACTION`
    /// while other databases use `BEGIN`). Savepoints are named `sp_{id}`.
    pub fn serialize_transaction(&self, op: &Transaction) -> String {
        let mut ret = String::new();
        let mut arg_positions = Vec::new();

        {
            let mut f = Formatter {
                serializer: self,
                cx: ExprContext::new(self.schema),
                dst: &mut ret,
                depth: 0,
                alias: false,
                in_insert: false,
                arg_positions: &mut arg_positions,
            };

            match op {
                Transaction::Start {
                    isolation,
                    read_only,
                    mode,
                } => fmt!(
                    &mut f,
                    self.serialize_transaction_start(*isolation, *read_only, *mode)
                ),
                Transaction::Commit => fmt!(&mut f, "COMMIT"),
                Transaction::Rollback => fmt!(&mut f, "ROLLBACK"),
                Transaction::Savepoint(name) => {
                    fmt!(&mut f, "SAVEPOINT " Ident(name))
                }
                Transaction::ReleaseSavepoint(name) => {
                    fmt!(&mut f, "RELEASE SAVEPOINT " Ident(name))
                }
                Transaction::RollbackToSavepoint(name) => {
                    fmt!(&mut f, "ROLLBACK TO SAVEPOINT " Ident(name))
                }
            };
        }

        ret.push(';');
        ret
    }

    fn serialize_transaction_start(
        &self,
        isolation: Option<IsolationLevel>,
        read_only: bool,
        mode: TransactionMode,
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
            // MySQL has no SQLite-style lock-mode keyword; drivers
            // reject non-Default `mode` before reaching the serializer.
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
            // PostgreSQL has no SQLite-style lock-mode keyword; drivers
            // reject non-Default `mode` before reaching the serializer.
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
            // SQLite has no per-transaction isolation level or read-only
            // keyword; the lock-acquisition mode is the only knob. `Default`
            // emits whatever the serializer was configured with at
            // construction (`BEGIN` by default, or e.g. `BEGIN CONCURRENT`
            // for Turso under MVCC). `Deferred`/`Immediate`/`Exclusive` are
            // explicit caller requests with fixed SQL.
            Flavor::Sqlite => match mode {
                TransactionMode::Default => self.sqlite_default_begin.to_string(),
                TransactionMode::Deferred => "BEGIN".to_string(),
                TransactionMode::Immediate => "BEGIN IMMEDIATE".to_string(),
                TransactionMode::Exclusive => "BEGIN EXCLUSIVE".to_string(),
            },
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
