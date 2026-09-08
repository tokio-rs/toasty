use crate::stmt::Value;

macro_rules! impl_net_conversions {
    ($ty:ty, $name:ident, $lit:literal) => {
        impl From<$ty> for Value {
            fn from(value: $ty) -> Self {
                Self::$name(value)
            }
        }

        impl TryFrom<Value> for $ty {
            type Error = crate::Error;

            fn try_from(value: Value) -> Result<Self, Self::Error> {
                match value {
                    Value::$name(value) => Ok(value),
                    _ => Err(crate::Error::type_conversion(value, $lit)),
                }
            }
        }
    };
}

impl_net_conversions!(cidr::IpCidr, Cidr, "cidr::IpCidr");
impl_net_conversions!(cidr::IpInet, Inet, "cidr::IpInet");
impl_net_conversions!(macaddr::MacAddr6, MacAddr, "macaddr::MacAddr6");
impl_net_conversions!(macaddr::MacAddr8, MacAddr8, "macaddr::MacAddr8");
