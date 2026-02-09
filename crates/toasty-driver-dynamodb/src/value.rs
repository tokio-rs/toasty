use aws_sdk_dynamodb::types::AttributeValue;
use toasty_core::{
    schema::app,
    stmt::{self, Value as CoreValue},
};

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
            (Type::Id(model), AV::S(val)) => {
                stmt::Value::from(stmt::Id::from_string(*model, val.clone()))
            }
            (Type::Enum(..), AV::S(val)) => {
                let (variant, rest) = val.split_once("#").unwrap();
                let variant: usize = variant.parse().unwrap();
                let v: V = serde_json::from_str(rest).unwrap();
                let value = match v {
                    V::Bool(v) => stmt::Value::Bool(v),
                    V::Null => stmt::Value::Null,
                    V::String(v) => stmt::Value::String(v),
                    V::Id(model, v) => {
                        stmt::Value::Id(stmt::Id::from_string(app::ModelId(model), v))
                    }
                    V::I8(v) => stmt::Value::I8(v),
                    V::I16(v) => stmt::Value::I16(v),
                    V::I32(v) => stmt::Value::I32(v),
                    V::I64(v) => stmt::Value::I64(v),
                    V::U8(v) => stmt::Value::U8(v),
                    V::U16(v) => stmt::Value::U16(v),
                    V::U32(v) => stmt::Value::U32(v),
                    V::U64(v) => stmt::Value::U64(v),
                };

                if value.is_null() {
                    stmt::ValueEnum {
                        variant,
                        fields: stmt::ValueRecord::from_vec(vec![]),
                    }
                    .into()
                } else {
                    stmt::ValueEnum {
                        variant,
                        fields: stmt::ValueRecord::from_vec(vec![value]),
                    }
                    .into()
                }
            }
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
            stmt::Value::Id(val) => AV::S(val.to_string()),
            stmt::Value::Enum(val) => {
                let v = match &val.fields[..] {
                    [] => V::Null,
                    [stmt::Value::Bool(v)] => V::Bool(*v),
                    [stmt::Value::String(v)] => V::String(v.to_string()),
                    [stmt::Value::I8(v)] => V::I8(*v),
                    [stmt::Value::I16(v)] => V::I16(*v),
                    [stmt::Value::I32(v)] => V::I32(*v),
                    [stmt::Value::I64(v)] => V::I64(*v),
                    [stmt::Value::U8(v)] => V::U8(*v),
                    [stmt::Value::U16(v)] => V::U16(*v),
                    [stmt::Value::U32(v)] => V::U32(*v),
                    [stmt::Value::U64(v)] => V::U64(*v),
                    [stmt::Value::Id(id)] => V::Id(id.model_id().0, id.to_string()),
                    _ => todo!("val={:#?}", val.fields),
                };
                AV::S(format!(
                    "{}#{}",
                    val.variant,
                    serde_json::to_string(&v).unwrap()
                ))
            }
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

#[derive(serde::Serialize, serde::Deserialize)]
enum V {
    Bool(bool),
    Null,
    String(String),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    Id(usize, String),
}
