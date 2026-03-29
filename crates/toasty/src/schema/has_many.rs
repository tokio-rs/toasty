use super::{Load, Relation, Scope};

use toasty_core::stmt::Value;

use std::fmt;

/// A lazily-loaded has-many association.
///
/// `HasMany<T>` wraps an optional `Vec<T>` that is populated when the
/// association is eagerly loaded (via `include`) or accessed through a
/// generated relation accessor. Before loading, calling
/// [`get`](HasMany::get) panics.
///
/// This type appears as a field on model structs for has-many relations.
#[derive(Clone)]
pub struct HasMany<T> {
    values: Option<Vec<T>>,
}

impl<T: Relation> Load for HasMany<T> {
    type Output = Self;

    fn ty() -> toasty_core::stmt::Type {
        toasty_core::stmt::Type::list(T::ty())
    }

    fn load(input: Value) -> crate::Result<Self> {
        match input {
            Value::List(items) => {
                let mut values = vec![];

                for value in items {
                    values.push(T::load_relation(value)?);
                }

                Ok(Self {
                    values: Some(values),
                })
            }
            Value::Null => Ok(Self::default()),
            _ => todo!("unexpected input: input={:#?}", input),
        }
    }
}

impl<T: Relation> HasMany<T> {
    /// Returns a slice of the loaded associated records.
    ///
    /// # Panics
    ///
    /// Panics if the association has not been loaded.
    #[track_caller]
    pub fn get(&self) -> &[T] {
        self.values
            .as_ref()
            .expect("association not loaded")
            .as_slice()
    }

    /// Returns `true` if the association has not been loaded yet.
    pub fn is_unloaded(&self) -> bool {
        self.values.is_none()
    }

    /// Clear the loaded values, returning this association to the unloaded
    /// state.
    pub fn unload(&mut self) {
        self.values = None;
    }
}

impl<T: Relation> Relation for HasMany<T> {
    type Model = T::Model;
    type Expr = T::Expr;
    type Query = T::Query;
    type Create = T::Create;
    type Many = T::Many;
    type ManyField<__Origin> = T::ManyField<__Origin>;
    type One = T::One;
    type OneField<__Origin> = T::OneField<__Origin>;
    type OptionOne = T::OptionOne;

    fn new_many_field<__Origin>(
        path: crate::stmt::Path<__Origin, crate::stmt::List<Self::Model>>,
    ) -> Self::ManyField<__Origin> {
        T::new_many_field(path)
    }

    fn field_name_to_id(name: &str) -> toasty_core::schema::app::FieldId {
        T::field_name_to_id(name)
    }

    fn nullable() -> bool {
        T::nullable()
    }
}

impl<T: Relation> Scope for HasMany<T> {
    type Item = crate::stmt::List<T::Model>;
    type Path<Origin> = T::ManyField<Origin>;
    type Create = T::Create;

    fn new_path<Origin>(path: crate::stmt::Path<Origin, Self::Item>) -> Self::Path<Origin> {
        T::new_many_field(path)
    }

    fn new_create() -> Self::Create {
        T::new_create()
    }

    fn new_path_root() -> Self::Path<Self::Item> {
        T::new_many_field(crate::stmt::Path::from_model_list())
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
