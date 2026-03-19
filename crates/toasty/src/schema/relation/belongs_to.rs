use crate::schema::relation::Relation;
use crate::Load;

use toasty_core::stmt::Value;

use std::fmt;

#[derive(Clone)]
pub struct BelongsTo<T> {
    value: Option<Box<T>>,
}

impl<T: Relation> Load for BelongsTo<T> {
    type Output = Self;
    fn load(input: Value) -> crate::Result<Self> {
        Ok(match input {
            Value::Null => Self::default(),
            value => Self {
                value: Some(Box::new(T::load(value)?)),
            },
        })
    }
}

impl<T: Relation> BelongsTo<T> {
    #[track_caller]
    pub fn get(&self) -> &T {
        self.value.as_ref().expect("association not loaded")
    }

    pub fn is_unloaded(&self) -> bool {
        self.value.is_none()
    }

    pub fn unload(&mut self) {
        self.value = None;
    }
}

impl<T: Relation> Relation for BelongsTo<T> {
    type Model = T::Model;
    type Create = T::Create;
    type Expr = T::Expr;
    type Query = T::Query;
    type Many = T::Many;
    type ManyField<__Origin> = T::ManyField<__Origin>;
    type One = T::One;
    type OneField<__Origin> = T::OneField<__Origin>;
    type OptionOne = T::OptionOne;

    fn field_name_to_id(name: &str) -> toasty_core::schema::app::FieldId {
        T::field_name_to_id(name)
    }

    fn nullable() -> bool {
        T::nullable()
    }
}

impl<T> Default for BelongsTo<T> {
    fn default() -> Self {
        Self { value: None }
    }
}

impl<T: fmt::Debug> fmt::Debug for BelongsTo<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value.as_ref() {
            Some(t) => t.fmt(fmt),
            None => {
                write!(fmt, "<not loaded>")?;
                Ok(())
            }
        }
    }
}

#[cfg(feature = "serde")]
impl<T: serde_core::Serialize> serde_core::Serialize for BelongsTo<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde_core::Serializer,
    {
        self.value.serialize(serializer)
    }
}
