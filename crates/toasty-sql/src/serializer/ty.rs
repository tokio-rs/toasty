use crate::serializer::ExprContext;

use super::{Flavor, ToSql};

use toasty_core::schema::db;

impl ToSql for &db::Type {
    fn to_sql(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_>) {
        match self {
            db::Type::Boolean => fmt!(cx, f, "BOOLEAN"),
            db::Type::Integer(1..=2) => fmt!(cx, f, "SMALLINT"),
            db::Type::Integer(3..=4) => fmt!(cx, f, "INTEGER"),
            db::Type::Integer(5..=8) => fmt!(cx, f, "BIGINT"),
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
            db::Type::Float(size) => match f.serializer.flavor {
                Flavor::Sqlite => fmt!(cx, f, "REAL"),
                Flavor::Postgresql => {
                    if *size <= 4 {
                        fmt!(cx, f, "REAL")
                    } else {
                        fmt!(cx, f, "DOUBLE PRECISION")
                    }
                }
                Flavor::Mysql => {
                    if *size <= 4 {
                        fmt!(cx, f, "FLOAT")
                    } else {
                        fmt!(cx, f, "DOUBLE")
                    }
                }
            },
            db::Type::Text => fmt!(cx, f, "TEXT"),
            db::Type::VarChar(size) => fmt!(cx, f, "VARCHAR(" size ")"),
            db::Type::Uuid => {
                fmt!(
                    cx,
                    f,
                    match f.serializer.flavor {
                        Flavor::Postgresql => "UUID",
                        _ => todo!("Unsupported type UUID"),
                    }
                );
            }
            db::Type::Numeric(None) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(cx, f, "NUMERIC"),
                Flavor::Mysql => todo!(
                    "MySQL does not support arbitrary-precision NUMERIC; precision and scale must be specified"
                ),
                Flavor::Sqlite => todo!("SQLite does not support NUMERIC type"),
            },
            db::Type::Numeric(Some((precision, scale))) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(cx, f, "NUMERIC(" precision ", " scale ")"),
                Flavor::Mysql => fmt!(cx, f, "DECIMAL(" precision ", " scale ")"),
                Flavor::Sqlite => todo!("SQLite does not support NUMERIC type"),
            },
            db::Type::Binary(size) => match f.serializer.flavor {
                Flavor::Mysql => fmt!(cx, f, "BINARY(" size ")"),
                _ => todo!("Unsupported fixed size binary type"),
            },
            db::Type::Blob => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(cx, f, "BYTEA"),
                Flavor::Mysql => fmt!(cx, f, "BLOB"),
                Flavor::Sqlite => fmt!(cx, f, "BLOB"),
            },
            db::Type::Timestamp(precision) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(cx, f, "TIMESTAMPTZ(" precision ")"),
                Flavor::Mysql => fmt!(cx, f, "TIMESTAMP(" precision ")"),
                Flavor::Sqlite => todo!("SQLite does not support Timestamp"),
            },
            db::Type::Date => match f.serializer.flavor {
                Flavor::Postgresql | Flavor::Mysql => fmt!(cx, f, "DATE"),
                Flavor::Sqlite => todo!("SQLite does not support Date"),
            },
            db::Type::Time(precision) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(cx, f, "TIME(" precision ")"),
                Flavor::Mysql => fmt!(cx, f, "TIME(" precision ")"),
                Flavor::Sqlite => todo!("SQLite does not support Time"),
            },
            db::Type::DateTime(precision) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(cx, f, "TIMESTAMP(" precision ")"),
                Flavor::Mysql => fmt!(cx, f, "DATETIME(" precision ")"),
                Flavor::Sqlite => todo!("SQLite does not support DateTime"),
            },
            db::Type::Enum(type_enum) => match f.serializer.flavor {
                // PostgreSQL: reference the named enum type created with CREATE TYPE.
                Flavor::Postgresql => {
                    let name = type_enum
                        .name
                        .as_deref()
                        .expect("PostgreSQL enums require a type name");
                    fmt!(cx, f, name);
                }
                // MySQL: inline ENUM('label1', 'label2', ...) column type.
                Flavor::Mysql => {
                    use toasty_core::stmt::Value;

                    f.dst.push_str("ENUM(");
                    for (i, variant) in type_enum.variants.iter().enumerate() {
                        if i > 0 {
                            f.dst.push_str(", ");
                        }
                        Value::String(variant.name.clone()).to_sql(cx, f);
                    }
                    f.dst.push(')');
                }
                // SQLite: TEXT column (CHECK constraint added in ColumnDef).
                Flavor::Sqlite => fmt!(cx, f, "TEXT"),
            },
            db::Type::Array(elem) => match f.serializer.flavor {
                Flavor::Postgresql => {
                    elem.as_ref().to_sql(cx, f);
                    f.dst.push_str("[]");
                }
                Flavor::Mysql | Flavor::Sqlite => {
                    todo!("Array column type is only supported on PostgreSQL")
                }
            },
            db::Type::Json => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(cx, f, "JSONB"),
                Flavor::Mysql => fmt!(cx, f, "JSON"),
                Flavor::Sqlite => fmt!(cx, f, "TEXT"),
            },
            db::Type::Custom(custom) => fmt!(cx, f, custom.as_str()),
        }
    }
}
