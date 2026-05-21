use super::Serializer;

use toasty_core::schema::db;

#[derive(Debug)]
pub(super) enum Flavor {
    Postgresql,
    Sqlite,
    Mysql,
}

impl<'a> Serializer<'a> {
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
            flavor: Flavor::Sqlite,
            sqlite_default_begin: default_begin,
        }
    }

    /// Returns `true` if this serializer targets SQLite.
    pub fn is_sqlite(&self) -> bool {
        matches!(self.flavor, Flavor::Sqlite)
    }

    /// Creates a serializer that emits PostgreSQL SQL.
    pub fn postgresql(schema: &'a db::Schema) -> Self {
        Serializer {
            schema,
            flavor: Flavor::Postgresql,
            sqlite_default_begin: "BEGIN",
        }
    }

    /// Creates a serializer that emits MySQL SQL.
    pub fn mysql(schema: &'a db::Schema) -> Self {
        Serializer {
            schema,
            flavor: Flavor::Mysql,
            sqlite_default_begin: "BEGIN",
        }
    }

    pub(super) fn is_mysql(&self) -> bool {
        matches!(self.flavor, Flavor::Mysql)
    }
}
