use super::Serializer;

use toasty_core::{
    driver::{Dialect, SqlPlaceholder},
    schema::db,
};

impl<'a> Serializer<'a> {
    /// Creates a serializer that emits SQLite SQL.
    pub fn sqlite(schema: &'a db::Schema) -> Self {
        Self::sqlite_with_default_begin(schema, "BEGIN")
    }

    /// Creates a SQLite-dialect serializer with a custom SQL string for
    /// [`TransactionMode::Default`].
    ///
    /// Used by SQLite-compatible engines whose preferred "no opinion" BEGIN
    /// is not the classic deferred form — e.g. Turso with
    /// `concurrent_writes()` enabled, where `Default` means `BEGIN
    /// CONCURRENT`. The non-`Default` modes (`Deferred`, `Immediate`,
    /// `Exclusive`) still map to their standard SQLite SQL.
    pub fn sqlite_with_default_begin(schema: &'a db::Schema, default_begin: &'static str) -> Self {
        Serializer {
            schema,
            dialect: Dialect::Sqlite,
            sqlite_default_begin: default_begin,
        }
    }

    /// Returns `true` if this serializer targets SQLite.
    pub fn is_sqlite(&self) -> bool {
        matches!(self.dialect, Dialect::Sqlite)
    }

    /// Creates a serializer that emits PostgreSQL SQL.
    pub fn postgresql(schema: &'a db::Schema) -> Self {
        Serializer {
            schema,
            dialect: Dialect::Postgresql,
            sqlite_default_begin: "BEGIN",
        }
    }

    /// Creates a serializer that emits MySQL SQL.
    pub fn mysql(schema: &'a db::Schema) -> Self {
        Serializer {
            schema,
            dialect: Dialect::Mysql,
            sqlite_default_begin: "BEGIN",
        }
    }

    pub(super) fn is_mysql(&self) -> bool {
        matches!(self.dialect, Dialect::Mysql)
    }
}

/// The placeholder syntax a dialect's bind layer accepts.
pub(super) fn sql_placeholder(dialect: Dialect) -> SqlPlaceholder {
    match dialect {
        Dialect::Postgresql => SqlPlaceholder::DollarNumber,
        Dialect::Sqlite => SqlPlaceholder::NumberedQuestionMark,
        Dialect::Mysql => SqlPlaceholder::QuestionMark,
    }
}
