use super::Serializer;

use toasty_core::{
    driver::{SqlFlavor, SqlPlaceholder},
    schema::db,
};

impl<'a> Serializer<'a> {
    /// Creates a serializer that emits SQL in `flavor`'s dialect.
    pub fn new(flavor: SqlFlavor, schema: &'a db::Schema) -> Self {
        match flavor {
            SqlFlavor::Sqlite => Self::sqlite(schema),
            SqlFlavor::Postgresql => Self::postgresql(schema),
            SqlFlavor::Mysql => Self::mysql(schema),
        }
    }

    /// Creates a serializer that emits SQLite SQL.
    pub fn sqlite(schema: &'a db::Schema) -> Self {
        Self::sqlite_with_default_begin(schema, "BEGIN")
    }

    /// Creates a SQLite-flavored serializer with a custom SQL string for
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
            flavor: SqlFlavor::Sqlite,
            sqlite_default_begin: default_begin,
        }
    }

    /// Returns `true` if this serializer targets SQLite.
    pub fn is_sqlite(&self) -> bool {
        matches!(self.flavor, SqlFlavor::Sqlite)
    }

    /// Creates a serializer that emits PostgreSQL SQL.
    pub fn postgresql(schema: &'a db::Schema) -> Self {
        Serializer {
            schema,
            flavor: SqlFlavor::Postgresql,
            sqlite_default_begin: "BEGIN",
        }
    }

    /// Creates a serializer that emits MySQL SQL.
    pub fn mysql(schema: &'a db::Schema) -> Self {
        Serializer {
            schema,
            flavor: SqlFlavor::Mysql,
            sqlite_default_begin: "BEGIN",
        }
    }

    pub(super) fn is_mysql(&self) -> bool {
        matches!(self.flavor, SqlFlavor::Mysql)
    }
}

/// The bind-parameter syntax `flavor` accepts.
pub(super) fn sql_placeholder(flavor: SqlFlavor) -> SqlPlaceholder {
    match flavor {
        SqlFlavor::Postgresql => SqlPlaceholder::DollarNumber,
        SqlFlavor::Sqlite => SqlPlaceholder::NumberedQuestionMark,
        SqlFlavor::Mysql => SqlPlaceholder::QuestionMark,
    }
}
