use mysql_async::prelude::ToValue;
use toasty_core::stmt::Value as CoreValue;

#[derive(Debug)]
pub struct Value(CoreValue);

impl From<CoreValue> for Value {
    fn from(value: CoreValue) -> Self {
        Self(value)
    }
}

impl ToValue for Value {
    fn to_value(&self) -> mysql_async::Value {
        match &self.0 {
            CoreValue::Bool(value) => value.to_value(),
            CoreValue::I32(value) => value.to_value(),
            CoreValue::I64(value) => value.to_value(),
            CoreValue::Id(id) => id.to_string().to_value(),
            CoreValue::Null => mysql_async::Value::NULL,
            CoreValue::String(value) => value.to_value(),
            value => todo!("{:#?}", value),
        }
    }
}
