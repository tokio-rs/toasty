use crate::Model;

use toasty_core::stmt::Value;

use std::fmt;

pub struct HasMany<T> {
    values: Option<Vec<T>>,
}

impl<T: Model> HasMany<T> {
    pub fn load(input: Value) -> crate::Result<Self> {
        match input {
            Value::Record(record) => {
                let mut values = vec![];

                for value in record.fields {
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
}

impl<T> Default for HasMany<T> {
    fn default() -> Self {
        Self { values: None }
    }
}

impl<T: fmt::Debug> fmt::Debug for HasMany<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_list().finish()
    }
}
