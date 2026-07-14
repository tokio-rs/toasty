use super::Load;
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
