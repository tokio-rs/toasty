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
        Serializer {
            schema,
            flavor: Flavor::Sqlite,
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
        }
    }

    /// Creates a serializer that emits MySQL SQL.
    pub fn mysql(schema: &'a db::Schema) -> Self {
        Serializer {
            schema,
            flavor: Flavor::Mysql,
        }
    }

    pub(super) fn is_mysql(&self) -> bool {
        matches!(self.flavor, Flavor::Mysql)
    }
}
