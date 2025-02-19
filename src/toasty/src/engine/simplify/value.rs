use super::*;

use stmt::Value;

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
