use super::{Params, ToSql};

use toasty_core::schema::db;

impl ToSql for &db::Type {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            db::Type::Boolean => fmt!(f, "BOOLEAN"),
            db::Type::Integer(1..=2) => fmt!(f, "SMALLINT"),
            db::Type::Integer(3..=4) => fmt!(f, "INTEGER"),
            db::Type::Integer(5..=8) => fmt!(f, "bigint"),
            db::Type::Integer(_) => todo!(),
            db::Type::Text => fmt!(f, "TEXT"),
            db::Type::VarChar(size) => fmt!(f, "VARCHAR(" size ")"),
        }
    }
}
