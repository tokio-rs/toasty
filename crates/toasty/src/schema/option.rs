use super::{Load, Relation};
use toasty_core::schema::app::FieldId;
use toasty_core::stmt::{self, Value};

impl<T: Load> Load for Option<T> {
    type Output = Option<T::Output>;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn ty_relation() -> stmt::Type {
        T::ty()
    }

    fn load(value: Value) -> Result<Self::Output, crate::Error> {
        match value {
            Value::Null => Ok(None),
            // Any other value is the raw model record (from INSERT or
            // SELECT+include when a matching row exists).
            v => Ok(Some(T::load(v)?)),
        }
    }

    fn load_relation(value: Value) -> Result<Self::Output, crate::Error> {
        match value {
            Value::Null => Ok(None),
            // Any other value is the raw model record (from INSERT or
            // SELECT+include when a matching row exists).
            v => Ok(Some(T::load(v)?)),
        }
    }

    fn reload(target: &mut Self::Output, value: Value) -> Result<(), crate::Error> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl<T: Relation> Relation for Option<T> {
    type Model = T::Model;
    type Expr = Option<T::Model>;
    type Query = T::Query;
    type Create = T::Create;
    type Many = T::Many;
    type ManyField<__Origin> = T::ManyField<__Origin>;
    type One = T::OptionOne;
    type OneField<__Origin> = T::OneField<__Origin>;
    type OptionOne = T::OptionOne;

    fn new_many_field<__Origin>(
        path: crate::stmt::Path<__Origin, crate::stmt::List<Self::Model>>,
    ) -> Self::ManyField<__Origin> {
        T::new_many_field(path)
    }

    fn field_name_to_id(name: &str) -> FieldId {
        T::field_name_to_id(name)
    }

    fn nullable() -> bool {
        true
    }
}
