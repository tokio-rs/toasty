use super::{Load, Register, Relation};

use toasty_core::schema::Name;
use toasty_core::schema::app::{self, FieldId, FieldTy, ModelId};
use toasty_core::stmt::{self, Value};

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

    fn ty() -> stmt::Type {
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

impl<T: Relation> Relation for HasOne<T> {
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

    fn has_one_field_ty(pair_hint: Option<Name>) -> FieldTy {
        FieldTy::HasOne(app::HasOne {
            target: <T::Model as Register>::id(),
            expr_ty: stmt::Type::Model(<T::Model as Register>::id()),
            // The pair is populated at runtime.
            pair: FieldId {
                model: ModelId(usize::MAX),
                index: usize::MAX,
            },
            pair_hint,
        })
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
