use super::Serializer;

use toasty_core::schema::db;

#[derive(Debug)]
pub(super) enum Flavor {
    Postgresql,
    Sqlite,
    Mysql,
}

impl<'a> Serializer<'a> {
    pub fn sqlite(schema: &'a db::Schema) -> Serializer<'a> {
        Serializer {
            schema,
            flavor: Flavor::Sqlite,
        }
    }

    pub fn postgresql(schema: &'a db::Schema) -> Serializer<'a> {
        Serializer {
            schema,
            flavor: Flavor::Postgresql,
        }
    }

    pub fn mysql(schema: &'a db::Schema) -> Serializer<'a> {
        Serializer {
            schema,
            flavor: Flavor::Mysql,
        }
    }
}
