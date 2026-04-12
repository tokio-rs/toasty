use toasty_core::{schema::db, stmt};
use tokio_postgres::types::Type;

pub trait TypeExt {
    /// Converts a database storage type to a PostgreSQL wire type.
    fn to_postgres_type(&self) -> Type;
}

impl TypeExt for db::Type {
    fn to_postgres_type(&self) -> Type {
        match self {
            db::Type::Boolean => Type::BOOL,
            db::Type::Integer(1) => Type::INT2,
            db::Type::Integer(2) => Type::INT2,
            db::Type::Integer(4) => Type::INT4,
            db::Type::Integer(8) => Type::INT8,
            db::Type::UnsignedInteger(1) => Type::INT2,
            db::Type::UnsignedInteger(2) => Type::INT4,
            db::Type::UnsignedInteger(4) => Type::INT8,
            db::Type::UnsignedInteger(8) => Type::INT8,
            db::Type::Text | db::Type::VarChar(_) => Type::TEXT,
            db::Type::Uuid => Type::UUID,
            db::Type::Numeric(_) => Type::NUMERIC,
            db::Type::Blob | db::Type::Binary(_) => Type::BYTEA,
            db::Type::Timestamp(_) => Type::TIMESTAMPTZ,
            db::Type::Date => Type::DATE,
            db::Type::Time(_) => Type::TIME,
            db::Type::DateTime(_) => Type::TIMESTAMP,
            // Enum types are handled separately via the cached OID map;
            // fall back to TEXT if we reach here (shouldn't happen in practice).
            db::Type::Enum(_) => Type::TEXT,
            _ => todo!("to_postgres_type; db_ty={:#?}", self),
        }
    }
}

/// Infers a PostgreSQL wire type from a `stmt::Value` when no storage type
/// is available (e.g. parameters without column context).
pub fn postgres_type_from_value(value: &stmt::Value) -> Type {
    match value {
        stmt::Value::Bool(_) => Type::BOOL,
        stmt::Value::I8(_) | stmt::Value::I16(_) => Type::INT2,
        stmt::Value::I32(_) => Type::INT4,
        stmt::Value::I64(_) => Type::INT8,
        stmt::Value::U8(_) | stmt::Value::U16(_) => Type::INT4,
        stmt::Value::U32(_) | stmt::Value::U64(_) => Type::INT8,
        stmt::Value::String(_) => Type::TEXT,
        stmt::Value::Uuid(_) => Type::UUID,
        stmt::Value::Bytes(_) => Type::BYTEA,
        #[cfg(feature = "rust_decimal")]
        stmt::Value::Decimal(_) => Type::NUMERIC,
        #[cfg(feature = "jiff")]
        stmt::Value::Timestamp(_) => Type::TIMESTAMPTZ,
        #[cfg(feature = "jiff")]
        stmt::Value::Date(_) => Type::DATE,
        #[cfg(feature = "jiff")]
        stmt::Value::Time(_) => Type::TIME,
        #[cfg(feature = "jiff")]
        stmt::Value::DateTime(_) => Type::TIMESTAMP,
        stmt::Value::Null => Type::TEXT,
        _ => todo!("postgres_type_from_value; value={:#?}", value),
    }
}
