use crate::Model;

use toasty_core::stmt::Value;

use std::fmt;

pub struct HasMany<T> {
    values: Option<Vec<T>>,
}

impl<T: Model> HasMany<T> {
    pub fn load(input: Value<'_>) -> crate::Result<HasMany<T>> {
        match input {
            Value::Record(record) => {
                let mut values = vec![];

                for value in record.into_owned() {
                    let Value::Record(record) = value else {
                        panic!("unexpected input; value={:#?}", value)
                    };

                    values.push(T::load(record.into_owned())?);
                }

                Ok(HasMany {
                    values: Some(values),
                })
            }
            Value::Null => Ok(HasMany::default()),
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
        HasMany { values: None }
    }
}

impl<T: fmt::Debug> fmt::Debug for HasMany<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_list().finish()
    }
}
