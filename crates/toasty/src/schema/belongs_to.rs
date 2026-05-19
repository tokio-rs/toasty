use super::{Load, Register, Relation};

use toasty_core::schema::app::{self, FieldTy, ForeignKey};
use toasty_core::stmt::{self, Value};

use std::fmt;

/// A lazily-loaded belongs-to association.
///
/// `BelongsTo<T>` wraps an optional `T` that is populated when the association
/// is eagerly loaded (via `include`) or accessed through a generated relation
/// accessor. Before loading, calling [`get`](BelongsTo::get) panics.
///
/// This type appears as a field on model structs for belongs-to relations.
#[derive(Clone)]
pub struct BelongsTo<T> {
    value: Option<Box<T>>,
}

impl<T: Relation> Load for BelongsTo<T> {
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

impl<T: Relation> BelongsTo<T> {
    /// Returns a reference to the loaded associated record.
    ///
    /// # Panics
    ///
    /// Panics if the association has not been loaded. Use [`try_get`] to
    /// handle the unloaded state without panicking.
    ///
    /// [`try_get`]: BelongsTo::try_get
    #[track_caller]
    pub fn get(&self) -> &T {
        self.value.as_ref().expect("association not loaded")
    }

    /// Returns a reference to the loaded associated record, or `None` if the
    /// association has not been loaded.
    ///
    /// This is the non-panicking counterpart to [`get`](BelongsTo::get). Use
    /// it when the caller cannot guarantee that the association was preloaded
    /// (for example, in code paths that may receive a record loaded without
    /// an `.include()`).
    pub fn try_get(&self) -> Option<&T> {
        self.value.as_deref()
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

impl<T: Relation> Relation for BelongsTo<T> {
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

    fn belongs_to_field_ty(foreign_key: ForeignKey) -> FieldTy {
        FieldTy::BelongsTo(app::BelongsTo {
            target: <T::Model as Register>::id(),
            expr_ty: stmt::Type::Model(<T::Model as Register>::id()),
            // The pair is populated at runtime.
            pair: None,
            foreign_key,
        })
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
