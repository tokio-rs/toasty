use crate::Model;

use toasty_core::stmt::Value;

use std::fmt;

pub struct HasOne<T> {
    value: Option<Box<T>>,
}

impl<T: Model> HasOne<T> {
    pub fn load(input: Value) -> crate::Result<HasOne<T>> {
        Ok(match input {
            Value::Null => HasOne::default(),
            Value::Record(record) => HasOne {
                value: Some(Box::new(T::load(record)?)),
            },
            _ => todo!(),
        })
    }

    #[track_caller]
    pub fn get(&self) -> &T {
        self.value.as_ref().expect("association not loaded")
    }
}

impl<T> Default for HasOne<T> {
    fn default() -> Self {
        HasOne { value: None }
    }
}

impl<T: fmt::Debug> fmt::Debug for HasOne<T> {
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
