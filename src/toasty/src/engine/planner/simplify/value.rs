use super::*;

use stmt::Value;

impl SimplifyExpr<'_> {
    pub(super) fn uncast_value_id(&self, value: &mut stmt::Value) {
        match value {
            Value::Id(id) => {
                *value = id.to_primitive().into();
            }
            Value::Null => {
                // Nothing to do
            }
            _ => todo!("{value:#?}"),
        }
    }
}
