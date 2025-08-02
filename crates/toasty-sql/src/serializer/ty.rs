use super::{Flavor, Params, ToSql};

use toasty_core::schema::db;

impl ToSql for &db::Type {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            db::Type::Boolean => fmt!(f, "BOOLEAN"),
            db::Type::Integer(1..=2) => fmt!(f, "SMALLINT"),
            db::Type::Integer(3..=4) => fmt!(f, "INTEGER"),
            db::Type::Integer(5..=8) => fmt!(f, "bigint"),
            db::Type::Integer(_) => todo!(),
            db::Type::UnsignedInteger(size) => {
                match f.serializer.flavor {
                    Flavor::Mysql => {
                        match size {
                            1 => fmt!(f, "TINYINT UNSIGNED"),
                            2 => fmt!(f, "SMALLINT UNSIGNED"),
                            3..=4 => fmt!(f, "INT UNSIGNED"),
                            5..=8 => fmt!(f, "BIGINT UNSIGNED"),
                            _ => todo!("Unsupported unsigned integer size: {}", size),
                        }
                    }
                    Flavor::Postgresql => {
                        match size {
                            1 => fmt!(f, "SMALLINT"), // u8 -> SMALLINT (i16)
                            2 => fmt!(f, "INTEGER"),  // u16 -> INTEGER (i32)
                            3..=4 => fmt!(f, "BIGINT"), // u32 -> BIGINT (i64)
                            5..=8 => fmt!(f, "NUMERIC"), // u64 -> NUMERIC (arbitrary precision)
                            _ => todo!("Unsupported unsigned integer size: {}", size),
                        }
                    }
                    Flavor::Sqlite => {
                        // SQLite uses INTEGER for all integer types
                        fmt!(f, "INTEGER")
                    }
                }
            }
            db::Type::Text => fmt!(f, "TEXT"),
            db::Type::VarChar(size) => fmt!(f, "VARCHAR(" size ")"),
        }
    }
}
