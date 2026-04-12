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
