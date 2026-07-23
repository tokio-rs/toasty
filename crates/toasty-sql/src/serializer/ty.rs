use super::{Flavor, ToSql};

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
                match f.serializer.flavor {
                    Flavor::Mysql => match size {
                        1 => fmt!(f, "TINYINT UNSIGNED"),
                        2 => fmt!(f, "SMALLINT UNSIGNED"),
                        3..=4 => fmt!(f, "INT UNSIGNED"),
                        5..=8 => fmt!(f, "BIGINT UNSIGNED"),
                        _ => todo!("Unsupported unsigned integer size: {}", size),
                    },
                    Flavor::Postgresql => {
                        match size {
                            1 => fmt!(f, "SMALLINT"),   // u8 -> SMALLINT (i16)
                            2 => fmt!(f, "INTEGER"),    // u16 -> INTEGER (i32)
                            3..=4 => fmt!(f, "BIGINT"), // u32 -> BIGINT (i64)
                            5..=8 => fmt!(f, "BIGINT"), // u64 -> BIGINT (i64) with capability limits
                            _ => todo!("Unsupported unsigned integer size: {}", size),
                        }
                    }
                    Flavor::Sqlite => {
                        // SQLite uses INTEGER for all integer types
                        fmt!(f, "INTEGER")
                    }
                }
            }
            db::Type::Float(size) => match f.serializer.flavor {
                Flavor::Sqlite => fmt!(f, "REAL"),
                Flavor::Postgresql => {
                    if *size <= 4 {
                        fmt!(f, "REAL")
                    } else {
                        fmt!(f, "DOUBLE PRECISION")
                    }
                }
                Flavor::Mysql => {
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
                    match f.serializer.flavor {
                        Flavor::Postgresql => "UUID",
                        _ => todo!("Unsupported type UUID"),
                    }
                );
            }
            db::Type::Numeric(None) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(f, "NUMERIC"),
                Flavor::Mysql => todo!(
                    "MySQL does not support arbitrary-precision NUMERIC; precision and scale must be specified"
                ),
                Flavor::Sqlite => todo!("SQLite does not support NUMERIC type"),
            },
            db::Type::Numeric(Some((precision, scale))) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(f, "NUMERIC(" precision ", " scale ")"),
                Flavor::Mysql => fmt!(f, "DECIMAL(" precision ", " scale ")"),
                Flavor::Sqlite => todo!("SQLite does not support NUMERIC type"),
            },
            db::Type::Binary(size) => match f.serializer.flavor {
                Flavor::Mysql => fmt!(f, "BINARY(" size ")"),
                _ => todo!("Unsupported fixed size binary type"),
            },
            db::Type::Blob => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(f, "BYTEA"),
                Flavor::Mysql => fmt!(f, "BLOB"),
                Flavor::Sqlite => fmt!(f, "BLOB"),
            },
            db::Type::Timestamp(precision) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(f, "TIMESTAMPTZ(" precision ")"),
                Flavor::Mysql => fmt!(f, "TIMESTAMP(" precision ")"),
                Flavor::Sqlite => todo!("SQLite does not support Timestamp"),
            },
            db::Type::Date => match f.serializer.flavor {
                Flavor::Postgresql | Flavor::Mysql => fmt!(f, "DATE"),
                Flavor::Sqlite => todo!("SQLite does not support Date"),
            },
            db::Type::Time(precision) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(f, "TIME(" precision ")"),
                Flavor::Mysql => fmt!(f, "TIME(" precision ")"),
                Flavor::Sqlite => todo!("SQLite does not support Time"),
            },
            db::Type::DateTime(precision) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(f, "TIMESTAMP(" precision ")"),
                Flavor::Mysql => fmt!(f, "DATETIME(" precision ")"),
                Flavor::Sqlite => todo!("SQLite does not support DateTime"),
            },
            db::Type::Enum(type_enum) => match f.serializer.flavor {
                // PostgreSQL: reference the named enum type created with CREATE TYPE.
                Flavor::Postgresql => {
                    let name = type_enum
                        .name
                        .as_deref()
                        .expect("PostgreSQL enums require a type name");
                    fmt!(f, name);
                }
                // MySQL: inline ENUM('label1', 'label2', ...) column type.
                Flavor::Mysql => {
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
                Flavor::Sqlite => fmt!(f, "TEXT"),
            },
            db::Type::List(elem) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(f, elem.as_ref() "[]"),
                // MySQL stores `Vec<scalar>` as a JSON document; SQLite uses
                // TEXT (JSON1 functions operate on either, but TEXT is the
                // idiomatic affinity). The element type is tracked by the
                // engine — it doesn't surface in the column DDL.
                Flavor::Mysql => fmt!(f, "JSON"),
                Flavor::Sqlite => fmt!(f, "TEXT"),
            },
            db::Type::Document { binary } => match f.serializer.flavor {
                // `binary` selects `jsonb` over `json` on PostgreSQL; the text
                // encoding (`#[document(text)]`) is not yet wired up.
                Flavor::Postgresql if *binary => fmt!(f, "JSONB"),
                Flavor::Postgresql => fmt!(f, "JSON"),
                Flavor::Mysql => fmt!(f, "JSON"),
                Flavor::Sqlite => fmt!(f, "TEXT"),
            },
            db::Type::Json => fmt!(f, "JSON"),
            db::Type::Jsonb => fmt!(f, "JSONB"),
            db::Type::Custom(custom) => fmt!(f, custom.as_str()),
        }
    }
}
