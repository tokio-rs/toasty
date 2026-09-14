use super::{Dialect, Ident, ToSql};

use toasty_core::schema::db;

impl ToSql for &db::Type {
    fn to_sql(self, f: &mut super::Formatter<'_>) {
        match self {
            db::Type::Boolean => fmt!(f, "BOOLEAN"),
            db::Type::Integer(1..=2) => fmt!(f, "SMALLINT"),
            db::Type::Integer(3..=4) => fmt!(f, "INTEGER"),
            db::Type::Integer(5..=8) => fmt!(f, "BIGINT"),
            db::Type::Integer(_) => todo!(),
            db::Type::UnsignedInteger(size) => {
                match f.serializer.dialect {
                    Dialect::Mysql => match size {
                        1 => fmt!(f, "TINYINT UNSIGNED"),
                        2 => fmt!(f, "SMALLINT UNSIGNED"),
                        3..=4 => fmt!(f, "INT UNSIGNED"),
                        5..=8 => fmt!(f, "BIGINT UNSIGNED"),
                        _ => todo!("Unsupported unsigned integer size: {}", size),
                    },
                    Dialect::Postgresql => {
                        match size {
                            1 => fmt!(f, "SMALLINT"),   // u8 -> SMALLINT (i16)
                            2 => fmt!(f, "INTEGER"),    // u16 -> INTEGER (i32)
                            3..=4 => fmt!(f, "BIGINT"), // u32 -> BIGINT (i64)
                            5..=8 => fmt!(f, "BIGINT"), // u64 -> BIGINT (i64) with capability limits
                            _ => todo!("Unsupported unsigned integer size: {}", size),
                        }
                    }
                    Dialect::Sqlite => {
                        // SQLite uses INTEGER for all integer types
                        fmt!(f, "INTEGER")
                    }
                }
            }
            db::Type::Float(size) => match f.serializer.dialect {
                Dialect::Sqlite => fmt!(f, "REAL"),
                Dialect::Postgresql => {
                    if *size <= 4 {
                        fmt!(f, "REAL")
                    } else {
                        fmt!(f, "DOUBLE PRECISION")
                    }
                }
                Dialect::Mysql => {
                    if *size <= 4 {
                        fmt!(f, "FLOAT")
                    } else {
                        fmt!(f, "DOUBLE")
                    }
                }
            },
            db::Type::Text => fmt!(f, "TEXT"),
            db::Type::VarChar(size) => fmt!(f, "VARCHAR(" size ")"),
            db::Type::Uuid => {
                fmt!(
                    f,
                    match f.serializer.dialect {
                        Dialect::Postgresql => "UUID",
                        _ => todo!("Unsupported type UUID"),
                    }
                );
            }
            db::Type::Numeric(None) => match f.serializer.dialect {
                Dialect::Postgresql => fmt!(f, "NUMERIC"),
                Dialect::Mysql => todo!(
                    "MySQL does not support arbitrary-precision NUMERIC; precision and scale must be specified"
                ),
                Dialect::Sqlite => todo!("SQLite does not support NUMERIC type"),
            },
            db::Type::Numeric(Some((precision, scale))) => match f.serializer.dialect {
                Dialect::Postgresql => fmt!(f, "NUMERIC(" precision ", " scale ")"),
                Dialect::Mysql => fmt!(f, "DECIMAL(" precision ", " scale ")"),
                Dialect::Sqlite => todo!("SQLite does not support NUMERIC type"),
            },
            db::Type::Binary(size) => match f.serializer.dialect {
                Dialect::Mysql => fmt!(f, "BINARY(" size ")"),
                _ => todo!("Unsupported fixed size binary type"),
            },
            db::Type::Blob => match f.serializer.dialect {
                Dialect::Postgresql => fmt!(f, "BYTEA"),
                Dialect::Mysql => fmt!(f, "BLOB"),
                Dialect::Sqlite => fmt!(f, "BLOB"),
            },
            db::Type::Timestamp(precision) => match f.serializer.dialect {
                Dialect::Postgresql => fmt!(f, "TIMESTAMPTZ(" precision ")"),
                Dialect::Mysql => fmt!(f, "TIMESTAMP(" precision ")"),
                Dialect::Sqlite => todo!("SQLite does not support Timestamp"),
            },
            db::Type::Date => match f.serializer.dialect {
                Dialect::Postgresql | Dialect::Mysql => fmt!(f, "DATE"),
                Dialect::Sqlite => todo!("SQLite does not support Date"),
            },
            db::Type::Time(precision) => match f.serializer.dialect {
                Dialect::Postgresql => fmt!(f, "TIME(" precision ")"),
                Dialect::Mysql => fmt!(f, "TIME(" precision ")"),
                Dialect::Sqlite => todo!("SQLite does not support Time"),
            },
            db::Type::DateTime(precision) => match f.serializer.dialect {
                Dialect::Postgresql => fmt!(f, "TIMESTAMP(" precision ")"),
                Dialect::Mysql => fmt!(f, "DATETIME(" precision ")"),
                Dialect::Sqlite => todo!("SQLite does not support DateTime"),
            },
            db::Type::Cidr => match f.serializer.dialect {
                Dialect::Postgresql => fmt!(f, "CIDR"),
                _ => todo!("Only PostgreSQL supports CIDR"),
            },
            db::Type::Inet => match f.serializer.dialect {
                Dialect::Postgresql => fmt!(f, "INET"),
                _ => todo!("Only PostgreSQL supports INET"),
            },
            db::Type::MacAddr => match f.serializer.dialect {
                Dialect::Postgresql => fmt!(f, "MACADDR"),
                _ => todo!("Only PostgreSQL supports MACADDR"),
            },
            db::Type::MacAddr8 => match f.serializer.dialect {
                Dialect::Postgresql => fmt!(f, "MACADDR8"),
                _ => todo!("Only PostgreSQL supports MACADDR8"),
            },
            db::Type::Enum(type_enum) => match f.serializer.dialect {
                // PostgreSQL: reference the named enum type created with CREATE TYPE.
                Dialect::Postgresql => {
                    let name = type_enum
                        .name
                        .as_deref()
                        .expect("PostgreSQL enums require a type name");
                    fmt!(f, Ident(name));
                }
                // MySQL: inline ENUM('label1', 'label2', ...) column type.
                Dialect::Mysql => {
                    use toasty_core::stmt::Value;

                    f.dst.push_str("ENUM(");
                    for (i, variant) in type_enum.variants.iter().enumerate() {
                        if i > 0 {
                            f.dst.push_str(", ");
                        }
                        Value::String(variant.name.clone()).to_sql(f);
                    }
                    f.dst.push(')');
                }
                // SQLite: TEXT column (CHECK constraint added in ColumnDef).
                Dialect::Sqlite => fmt!(f, "TEXT"),
            },
            db::Type::List(elem) => match f.serializer.dialect {
                Dialect::Postgresql => fmt!(f, elem.as_ref() "[]"),
                // MySQL stores `Vec<scalar>` as a JSON document; SQLite uses
                // TEXT (JSON1 functions operate on either, but TEXT is the
                // idiomatic affinity). The element type is tracked by the
                // engine — it doesn't surface in the column DDL.
                Dialect::Mysql => fmt!(f, "JSON"),
                Dialect::Sqlite => fmt!(f, "TEXT"),
            },
            db::Type::Document { binary } => match f.serializer.dialect {
                // `binary` selects `jsonb` over `json` on PostgreSQL; the text
                // encoding (`#[document(text)]`) is not yet wired up.
                Dialect::Postgresql if *binary => fmt!(f, "JSONB"),
                Dialect::Postgresql => fmt!(f, "JSON"),
                Dialect::Mysql => fmt!(f, "JSON"),
                Dialect::Sqlite => fmt!(f, "TEXT"),
            },
            db::Type::Json => fmt!(f, "JSON"),
            db::Type::Jsonb => fmt!(f, "JSONB"),
            db::Type::Custom(custom) => fmt!(f, custom.as_str()),
        }
    }
}
