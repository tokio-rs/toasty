use super::{Create, Load, Relation};
use toasty_core::schema::app::FieldId;
use toasty_core::stmt::{self, Value};

impl<T: Load> Load for Option<T> {
    type Output = Option<T::Output>;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn ty_relation() -> stmt::Type {
        let ty = T::ty();

        debug_assert!(!ty.is_u64());

        let mut union = stmt::TypeUnion::new();
        union.insert(stmt::Type::I64);
        union.insert(ty);
        union.into()
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

impl<T: Relation> Create<T::Model> for Option<T> {
    type Builder = <T as Create<T::Model>>::Builder;
}

impl<T: Relation> Relation for Option<T> {
    type Model = T::Model;
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
