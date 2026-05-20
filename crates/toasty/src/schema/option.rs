use super::{Load, Relation};
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
        // Decodes a nullable wrapper's "loaded as None" state for both relation
        // targets and deferred fields (see the contract on
        // [`Load::load_relation`]). The sentinel takes one of two forms:
        //
        // - Nullable single relations (`BelongsTo<Option<_>>` /
        //   `HasOne<Option<_>>`) encode a missing row as `I64(0)`. The include
        //   lowering rewrites the subquery's `Null` result to `I64(0)` so it
        //   stays distinct from the `Null` that means "unloaded", which the
        //   relation wrapper's `load` handles before calling this method.
        // - Nullable deferred fields (`Deferred<Option<_>>`) wrap the value in
        //   a single-field record, so a loaded `None` reaches this method as a
        //   bare `Null` after `Deferred::load` reads that field.
        match value {
            Value::I64(0) | Value::Null => Ok(None),
            // Any other value is the present inner value: a model record for a
            // relation, or the column value for a deferred field.
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
