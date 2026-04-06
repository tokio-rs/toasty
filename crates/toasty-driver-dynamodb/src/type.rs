use aws_sdk_dynamodb::types::ScalarAttributeType;
use toasty_core::stmt;

pub trait TypeExt {
    /// Converts a Toasty type to a DynamoDB scalar type.
    fn to_ddb_type(&self) -> ScalarAttributeType;
}

impl TypeExt for stmt::Type {
    fn to_ddb_type(&self) -> ScalarAttributeType {
        match self {
            stmt::Type::Bool => ScalarAttributeType::N,
            stmt::Type::String => ScalarAttributeType::S,
            stmt::Type::I8
            | stmt::Type::I16
            | stmt::Type::I32
            | stmt::Type::I64
            | stmt::Type::U8
            | stmt::Type::U16
            | stmt::Type::U32
            | stmt::Type::U64 => ScalarAttributeType::N,
            stmt::Type::Bytes => ScalarAttributeType::B,
            _ => panic!("key attribute must be a string, number, or binary"),
        }
    }
}
