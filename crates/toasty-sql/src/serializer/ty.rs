use super::{Params, ToSql};

use toasty_core::schema::db;

impl ToSql for &db::Type {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            db::Type::Boolean => fmt!(f, "BOOLEAN"),
            db::Type::Integer => fmt!(f, "INTEGER"),
            db::Type::Text => fmt!(f, "TEXT"),
            db::Type::VarChar(size) => fmt!(f, "VARCHAR(" size ")"),
        }
    }
}
