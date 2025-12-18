use super::Serializer;

use toasty_core::schema::db;

#[derive(Debug)]
pub(super) enum Flavor {
    Postgresql,
    Sqlite,
    Mysql,
}

impl<'a> Serializer<'a> {
    pub fn sqlite(schema: &'a db::Schema) -> Self {
        Serializer {
            schema,
            flavor: Flavor::Sqlite,
        }
    }

    pub fn is_sqlite(&self) -> bool {
        matches!(self.flavor, Flavor::Sqlite)
    }

    pub fn postgresql(schema: &'a db::Schema) -> Self {
        Serializer {
            schema,
            flavor: Flavor::Postgresql,
        }
    }

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
