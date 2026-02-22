use aws_sdk_dynamodb::types::AttributeValue;
use toasty_core::stmt::{self, Value as CoreValue};

#[derive(Debug)]
pub struct Value(CoreValue);

impl From<CoreValue> for Value {
    fn from(value: CoreValue) -> Self {
        Self(value)
    }
}

impl Value {
    /// Converts this DynamoDB driver value into the core Toasty value.
    pub fn into_inner(self) -> CoreValue {
        self.0
    }

    /// Converts a DynamoDB AttributeValue to a Toasty value.
    pub fn from_ddb(ty: &stmt::Type, val: &AttributeValue) -> Self {
        use stmt::Type;
        use AttributeValue as AV;

        let core_value = match (ty, val) {
            (Type::Bool, AV::Bool(val)) => stmt::Value::from(*val),
            (Type::String, AV::S(val)) => stmt::Value::from(val.clone()),
            (Type::I8, AV::N(val)) => stmt::Value::from(val.parse::<i8>().unwrap()),
            (Type::I16, AV::N(val)) => stmt::Value::from(val.parse::<i16>().unwrap()),
            (Type::I32, AV::N(val)) => stmt::Value::from(val.parse::<i32>().unwrap()),
            (Type::I64, AV::N(val)) => stmt::Value::from(val.parse::<i64>().unwrap()),
            (Type::U8, AV::N(val)) => stmt::Value::from(val.parse::<u8>().unwrap()),
            (Type::U16, AV::N(val)) => stmt::Value::from(val.parse::<u16>().unwrap()),
            (Type::U32, AV::N(val)) => stmt::Value::from(val.parse::<u32>().unwrap()),
            (Type::U64, AV::N(val)) => stmt::Value::from(val.parse::<u64>().unwrap()),
            (Type::Bytes, AV::B(val)) => stmt::Value::Bytes(val.clone().into_inner()),
            (Type::Uuid, AV::S(val)) => stmt::Value::from(val.parse::<uuid::Uuid>().unwrap()),
            _ => todo!("ty={:#?}; value={:#?}", ty, val),
        };

        Value(core_value)
    }

    /// Converts this value to a DynamoDB AttributeValue.
    pub fn to_ddb(&self) -> AttributeValue {
        use AttributeValue as AV;

        match &self.0 {
            stmt::Value::Bool(val) => AV::Bool(*val),
            stmt::Value::String(val) => AV::S(val.to_string()),
            stmt::Value::I8(val) => AV::N(val.to_string()),
            stmt::Value::I16(val) => AV::N(val.to_string()),
            stmt::Value::I32(val) => AV::N(val.to_string()),
            stmt::Value::I64(val) => AV::N(val.to_string()),
            stmt::Value::U8(val) => AV::N(val.to_string()),
            stmt::Value::U16(val) => AV::N(val.to_string()),
            stmt::Value::U32(val) => AV::N(val.to_string()),
            stmt::Value::U64(val) => AV::N(val.to_string()),
            stmt::Value::Bytes(val) => AV::B(val.clone().into()),
            stmt::Value::Uuid(val) => AV::S(val.to_string()),
            stmt::Value::List(vals) => {
                let items = vals
                    .iter()
                    .map(|val| Value(val.clone()).to_ddb())
                    .collect::<Vec<_>>();
                AV::L(items)
            }
            _ => todo!("{:#?}", self.0),
        }
    }
}
