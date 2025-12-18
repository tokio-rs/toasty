use crate::{relation::Relation, Model};

use toasty_core::stmt::Value;

use std::fmt;

#[derive(Clone)]
pub struct HasMany<T> {
    values: Option<Vec<T>>,
}

impl<T: Model> HasMany<T> {
    pub fn load(input: Value) -> crate::Result<Self> {
        match input {
            Value::List(items) => {
                let mut values = vec![];

                for value in items {
                    let Value::Record(record) = value else {
                        panic!("unexpected input; value={value:#?}")
                    };

                    values.push(T::load(record)?);
                }

                Ok(Self {
                    values: Some(values),
                })
            }
            Value::Null => Ok(Self::default()),
            _ => todo!("unexpected input: input={:#?}", input),
        }
    }

    #[track_caller]
    pub fn get(&self) -> &[T] {
        self.values
            .as_ref()
            .expect("association not loaded")
            .as_slice()
    }

    pub fn is_unloaded(&self) -> bool {
        self.values.is_none()
    }

    pub fn unload(&mut self) {
        self.values = None;
    }
}

impl<T: Relation> Relation for HasMany<T> {
    type Model = T::Model;
    type Expr = T::Expr;
    type Query = T::Query;
    type Many = T::Many;
    type ManyField = T::ManyField;
    type One = T::One;
    type OneField = T::OneField;
    type OptionOne = T::OptionOne;

    fn field_name_to_id(name: &str) -> toasty_core::schema::app::FieldId {
        T::field_name_to_id(name)
    }

    fn nullable() -> bool {
        T::nullable()
    }
}

impl<T> Default for HasMany<T> {
    fn default() -> Self {
        Self { values: None }
    }
}

impl<T: fmt::Debug> fmt::Debug for HasMany<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.values.as_ref() {
            Some(t) => t.fmt(fmt),
            None => {
                write!(fmt, "<not loaded>")?;
                Ok(())
            }
        }
    }
}

#[cfg(feature = "serde")]
impl<T: serde_core::Serialize> serde_core::Serialize for HasMany<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde_core::Serializer,
    {
        self.values.serialize(serializer)
    }
}
