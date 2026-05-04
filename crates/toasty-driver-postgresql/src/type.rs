use toasty_core::schema::db;
use tokio_postgres::types::Type;

pub trait TypeExt {
    /// Converts a database storage type to a PostgreSQL wire type.
    fn to_postgres_type(&self) -> Type;
}

impl TypeExt for db::Type {
    fn to_postgres_type(&self) -> Type {
        match self {
            db::Type::Boolean => Type::BOOL,
            db::Type::Float(4) => Type::FLOAT4,
            db::Type::Float(8) => Type::FLOAT8,
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
            db::Type::Array(elem) => match elem.to_postgres_type() {
                Type::BOOL => Type::BOOL_ARRAY,
                Type::INT2 => Type::INT2_ARRAY,
                Type::INT4 => Type::INT4_ARRAY,
                Type::INT8 => Type::INT8_ARRAY,
                Type::FLOAT4 => Type::FLOAT4_ARRAY,
                Type::FLOAT8 => Type::FLOAT8_ARRAY,
                Type::TEXT | Type::VARCHAR => Type::TEXT_ARRAY,
                Type::UUID => Type::UUID_ARRAY,
                Type::NUMERIC => Type::NUMERIC_ARRAY,
                other => todo!("to_postgres_type: array of {other:#?} unsupported"),
            },
            db::Type::Json => Type::JSONB,
            _ => todo!("to_postgres_type; db_ty={:#?}", self),
        }
    }
}
