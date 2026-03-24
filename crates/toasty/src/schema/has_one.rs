use super::{Create, Load, Relation};

use toasty_core::stmt::Value;

use std::fmt;

/// A lazily-loaded has-one association.
///
/// `HasOne<T>` wraps an optional `T` that is populated when the association
/// is eagerly loaded (via `include`) or accessed through a generated relation
/// accessor. Before loading, calling [`get`](HasOne::get) panics.
///
/// This type appears as a field on model structs for has-one relations.
#[derive(Clone)]
pub struct HasOne<T> {
    value: Option<Box<T>>,
}

impl<T: Relation> Load for HasOne<T> {
    type Output = Self;

    fn ty() -> toasty_core::stmt::Type {
        T::ty_relation()
    }

    fn load(input: Value) -> crate::Result<Self> {
        Ok(match input {
            Value::Null => Self::default(),
            value => Self {
                value: Some(Box::new(T::load_relation(value)?)),
            },
        })
    }
}

impl<T: Relation> HasOne<T> {
    /// Returns a reference to the loaded associated record.
    ///
    /// # Panics
    ///
    /// Panics if the association has not been loaded.
    #[track_caller]
    pub fn get(&self) -> &T {
        self.value.as_ref().expect("association not loaded")
    }

    /// Returns `true` if the association has not been loaded yet.
    pub fn is_unloaded(&self) -> bool {
        self.value.is_none()
    }

    /// Clear the loaded value, returning this association to the unloaded
    /// state.
    pub fn unload(&mut self) {
        self.value = None;
    }
}

impl<T: Relation> Create<T::Model> for HasOne<T> {
    type Builder = <T as Create<T::Model>>::Builder;
}

impl<T: Relation> Relation for HasOne<T> {
    type Model = T::Model;
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

impl<T> Default for HasOne<T> {
    fn default() -> Self {
        Self { value: None }
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

#[cfg(feature = "serde")]
impl<T: serde_core::Serialize> serde_core::Serialize for HasOne<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde_core::Serializer,
    {
        self.value.serialize(serializer)
    }
}
