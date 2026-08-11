use crate::{
    Result,
    stmt::{Type, Value},
};

impl Type {
    /// Casts between network address values and their canonical text forms.
    pub fn cast_net(&self, value: &Value) -> Result<Option<Value>> {
        Ok(Some(match (value, self) {
            (Value::String(value), Type::Cidr) => Value::Cidr(parse(value, "cidr::IpCidr")?),
            (Value::String(value), Type::Inet) => Value::Inet(parse(value, "cidr::IpInet")?),
            (Value::String(value), Type::MacAddr) => {
                Value::MacAddr(parse(value, "macaddr::MacAddr6")?)
            }
            (Value::String(value), Type::MacAddr8) => {
                Value::MacAddr8(parse(value, "macaddr::MacAddr8")?)
            }

            (value @ Value::Cidr(_), Type::Cidr)
            | (value @ Value::Inet(_), Type::Inet)
            | (value @ Value::MacAddr(_), Type::MacAddr)
            | (value @ Value::MacAddr8(_), Type::MacAddr8) => value.clone(),

            (Value::Cidr(value), Type::String) => Value::String(value.to_string()),
            (Value::Inet(value), Type::String) => Value::String(value.to_string()),
            (Value::MacAddr(value), Type::String) => Value::String(value.to_string()),
            (Value::MacAddr8(value), Type::String) => Value::String(value.to_string()),

            _ => return Ok(None),
        }))
    }
}

fn parse<T>(value: &str, target: &'static str) -> Result<T>
where
    T: std::str::FromStr,
{
    value
        .parse()
        .map_err(|_| crate::Error::type_conversion(Value::String(value.to_owned()), target))
}
