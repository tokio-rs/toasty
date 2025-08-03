use postgres::types::{accepts, private::BytesMut, to_sql_checked, IsNull, ToSql, Type};
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
                _ => todo!("Unsupported PostgreSQL type for u16: {:?}", ty),
            },
            stmt::Value::U32(value) => match *ty {
                Type::INT8 => {
                    // u32 stored in BIGINT
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!("Unsupported PostgreSQL type for u32: {:?}", ty),
            },
            stmt::Value::U64(value) => match *ty {
                Type::INT8 => {
                    // PostgreSQL BIGINT is signed, so validate u64 fits in i64 range
                    if *value > i64::MAX as u64 {
                        return Err(Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("u64 value {} exceeds i64::MAX ({}), cannot store in PostgreSQL BIGINT", 
                                value, i64::MAX)
                        )));
                    }
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!("Unsupported PostgreSQL type for u64: {:?}", ty),
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
