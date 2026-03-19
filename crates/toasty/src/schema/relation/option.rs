use crate::schema::relation::Relation;
use crate::Load;
use toasty_core::schema::app::FieldId;
use toasty_core::stmt::Value;

impl<T: Relation> Load for Option<T> {
    type Output = Self;
    fn load(value: Value) -> Result<Self, crate::Error> {
        match value {
            // Encoded "loaded as None" from SELECT+include path.
            // The nested merge's Match expression transforms Value::Null
            // (no matching row) into I64(0) to distinguish from
            // Value::Null (unloaded), which HasOne::load handles.
            Value::I64(0) => Ok(None),
            // Any other value is the raw model record (from INSERT or
            // SELECT+include when a matching row exists).
            v => Ok(Some(T::load(v)?)),
        }
    }
}

impl<T: Relation> Relation for Option<T> {
    type Model = T::Model;
    type Create = T::Create;
    type Expr = Option<T::Model>;
    type Query = T::Query;
    type Many = T::Many;
    type ManyField<__Origin> = T::ManyField<__Origin>;
    type One = T::OptionOne;
    type OneField<__Origin> = T::OneField<__Origin>;
    type OptionOne = T::OptionOne;

    fn field_name_to_id(name: &str) -> FieldId {
        T::field_name_to_id(name)
    }

    fn nullable() -> bool {
        true
    }
}
