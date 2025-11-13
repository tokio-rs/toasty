use crate::serializer::ExprContext;

use super::{Flavor, Params, ToSql};

use toasty_core::schema::db;

impl ToSql for &db::Type {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        match self {
            db::Type::Boolean => fmt!(cx, f, "BOOLEAN"),
            db::Type::Integer(1..=2) => fmt!(cx, f, "SMALLINT"),
            db::Type::Integer(3..=4) => fmt!(cx, f, "INTEGER"),
            db::Type::Integer(5..=8) => fmt!(cx, f, "bigint"),
            db::Type::Integer(_) => todo!(),
            db::Type::UnsignedInteger(size) => {
                match f.serializer.flavor {
                    Flavor::Mysql => match size {
                        1 => fmt!(cx, f, "TINYINT UNSIGNED"),
                        2 => fmt!(cx, f, "SMALLINT UNSIGNED"),
                        3..=4 => fmt!(cx, f, "INT UNSIGNED"),
                        5..=8 => fmt!(cx, f, "BIGINT UNSIGNED"),
                        _ => todo!("Unsupported unsigned integer size: {}", size),
                    },
                    Flavor::Postgresql => {
                        match size {
                            1 => fmt!(cx, f, "SMALLINT"),   // u8 -> SMALLINT (i16)
                            2 => fmt!(cx, f, "INTEGER"),    // u16 -> INTEGER (i32)
                            3..=4 => fmt!(cx, f, "BIGINT"), // u32 -> BIGINT (i64)
                            5..=8 => fmt!(cx, f, "BIGINT"), // u64 -> BIGINT (i64) with capability limits
                            _ => todo!("Unsupported unsigned integer size: {}", size),
                        }
                    }
                    Flavor::Sqlite => {
                        // SQLite uses INTEGER for all integer types
                        fmt!(cx, f, "INTEGER")
                    }
                }
            }
            db::Type::Text => fmt!(cx, f, "TEXT"),
            db::Type::VarChar(size) => fmt!(cx, f, "VARCHAR(" size ")"),
            db::Type::Custom(custom) => fmt!(cx, f, custom.as_str()),
        }
    }
}
