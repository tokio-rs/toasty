use postgres::types::Type;
use toasty_core::stmt;

pub trait TypeExt {
    /// Converts a Toasty type to a PostgreSQL type.
    fn to_postgres_type(&self) -> Type;
}

impl TypeExt for stmt::Type {
    fn to_postgres_type(&self) -> Type {
        match self {
            stmt::Type::Null => Type::TEXT, // Default for NULL values

            stmt::Type::Bool => Type::BOOL,
            stmt::Type::I8 => Type::INT2,
            stmt::Type::I16 => Type::INT2,
            stmt::Type::I32 => Type::INT4,
            stmt::Type::I64 => Type::INT8,
            stmt::Type::U8 => Type::INT2,
            stmt::Type::U16 => Type::INT4,
            stmt::Type::U32 => Type::INT8,
            stmt::Type::U64 => Type::INT8,
            stmt::Type::Id(_) => Type::TEXT,
            stmt::Type::String => Type::TEXT,
            stmt::Type::Uuid => Type::UUID,
            stmt::Type::Bytes => Type::BYTEA,
            #[cfg(feature = "rust_decimal")]
            stmt::Type::Decimal => Type::NUMERIC,
            #[cfg(feature = "jiff")]
            stmt::Type::JiffTimestamp => Type::TIMESTAMPTZ,
            #[cfg(feature = "jiff")]
            stmt::Type::JiffDate => Type::DATE,
            #[cfg(feature = "jiff")]
            stmt::Type::JiffTime => Type::TIME,
            #[cfg(feature = "jiff")]
            stmt::Type::JiffDateTime => Type::TIMESTAMP,
            #[cfg(feature = "chrono")]
            stmt::Type::ChronoDateTimeUtc => Type::TIMESTAMPTZ,
            #[cfg(feature = "chrono")]
            stmt::Type::ChronoNaiveDateTime => Type::TIMESTAMP,
            #[cfg(feature = "chrono")]
            stmt::Type::ChronoNaiveDate => Type::DATE,
            #[cfg(feature = "chrono")]
            stmt::Type::ChronoNaiveTime => Type::TIME,

            _ => todo!("to_postgres_type; ty={:#?}", self),
        }
    }
}
