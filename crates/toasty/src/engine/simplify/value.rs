use super::Simplify;
use toasty_core::stmt::{self, Value};

impl Simplify<'_> {
    pub(super) fn uncast_value_id(&self, value: &mut stmt::Value) {
        match value {
            Value::Id(id) => {
                *value = id.to_primitive();
            }
            Value::Null => {
                // Nothing to do
            }
            _ => todo!("{value:#?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::test::test_schema;
    use toasty_core::schema::app::ModelId;
    use toasty_core::stmt::Id;

    // TODO: test int id uncasting when implemented

    #[test]
    fn id_string_becomes_string() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `uncast_value_id(Id("abc123")) → "abc123"`
        let mut value = Value::Id(Id::from_string(ModelId(0), "abc123".to_string()));
        simplify.uncast_value_id(&mut value);

        assert!(matches!(value, Value::String(s) if s == "abc123"));
    }

    #[test]
    fn null_stays_null() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `uncast_value_id(null) → null`
        let mut value = Value::Null;
        simplify.uncast_value_id(&mut value);

        assert!(matches!(value, Value::Null));
    }
}
