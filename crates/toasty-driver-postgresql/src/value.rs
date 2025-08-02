use postgres::types::{accepts, private::BytesMut, to_sql_checked, IsNull, ToSql, Type};
use rust_decimal::Decimal;
use toasty_core::stmt::{self, Value as CoreValue};

#[derive(Debug)]
pub struct Value(CoreValue);

impl From<CoreValue> for Value {
    fn from(value: CoreValue) -> Self {
        Self(value)
    }
}

impl ToSql for Value {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> std::result::Result<IsNull, Box<dyn std::error::Error + Sync + Send>>
    where
        Self: Sized,
    {
        match &self.0 {
            stmt::Value::Bool(value) => value.to_sql(ty, out),
            stmt::Value::I8(value) => match *ty {
                Type::INT2 => {
                    let value = *value as i16;
                    value.to_sql(ty, out)
                }
                Type::INT4 => {
                    let value = *value as i32;
                    value.to_sql(ty, out)
                }
                Type::INT8 => {
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!(),
            },
            stmt::Value::I16(value) => match *ty {
                Type::INT2 => value.to_sql(ty, out),
                Type::INT4 => {
                    let value = *value as i32;
                    value.to_sql(ty, out)
                }
                Type::INT8 => {
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!(),
            },
            stmt::Value::I32(value) => match *ty {
                Type::INT4 => value.to_sql(ty, out),
                Type::INT8 => {
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!(),
            },
            // TODO: we need to do better type management
            stmt::Value::I64(value) => match *ty {
                Type::INT4 => {
                    let value = *value as i32;
                    value.to_sql(ty, out)
                }
                Type::INT8 => value.to_sql(ty, out),
                _ => todo!(),
            },
            stmt::Value::U8(value) => match *ty {
                Type::INT2 => {
                    // u8 is now stored in i16 (SMALLINT)
                    let value = *value as i16;
                    value.to_sql(ty, out)
                }
                Type::INT4 => {
                    let value = *value as i32;
                    value.to_sql(ty, out)
                }
                Type::INT8 => {
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!(),
            },
            stmt::Value::U16(value) => match *ty {
                Type::INT4 => {
                    // u16 is now stored in i32 (INTEGER)
                    let value = *value as i32;
                    value.to_sql(ty, out)
                }
                Type::INT8 => {
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!("u16 should not be stored in INT2 anymore: {:?}", ty),
            },
            stmt::Value::U32(value) => match *ty {
                Type::INT8 => {
                    // u32 is now stored in i64 (BIGINT)
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!("u32 should not be stored in INT4 anymore: {:?}", ty),
            },
            stmt::Value::U64(value) => match *ty {
                Type::NUMERIC => {
                    // Convert u64 to Decimal for proper NUMERIC storage
                    let decimal = Decimal::from(*value);
                    decimal.to_sql(ty, out)
                }
                Type::INT8 => {
                    // Fallback for existing schemas - validate range
                    if *value > i64::MAX as u64 {
                        return Err(Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("u64 value {} exceeds i64::MAX ({}), cannot store in PostgreSQL BIGINT. Use NUMERIC column type for full u64 range.", 
                                value, i64::MAX)
                        )));
                    }
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!(),
            },
            stmt::Value::Id(value) => value.to_string().to_sql(ty, out),
            stmt::Value::Null => Ok(IsNull::Yes),
            stmt::Value::String(value) => value.to_sql(ty, out),
            value => todo!("{value:#?}"),
        }
    }

    accepts!(BOOL, INT2, INT4, INT8, NUMERIC, TEXT, VARCHAR);
    to_sql_checked!();
}
