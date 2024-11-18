use crate::Model;

use toasty_core::stmt::Value;

use std::fmt;

pub struct BelongsTo<T> {
    value: Option<T>,
}

impl<T: Model> BelongsTo<T> {
    pub fn load(input: Value<'_>) -> crate::Result<BelongsTo<T>> {
        Ok(match input {
            Value::Null => BelongsTo::default(),
            Value::Record(record) => BelongsTo {
                value: Some(T::load(record)?),
            },
            _ => todo!(),
        })
    }

    #[track_caller]
    pub fn get(&self) -> &T {
        self.value.as_ref().expect("association not loaded")
    }
}

impl<T> Default for BelongsTo<T> {
    fn default() -> Self {
        BelongsTo { value: None }
    }
}

impl<T: fmt::Debug> fmt::Debug for BelongsTo<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value.as_ref() {
            Some(t) => t.fmt(fmt),
            None => Ok(()),
        }
    }
}
