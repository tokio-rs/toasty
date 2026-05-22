use super::has_many::has_kind;
use super::{Load, Register, Relation, lazy_slot};

use toasty_core::schema::app::{self, FieldId, FieldTy};
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
        load_single_relation(input, "has-one relation")
    }
}

fn load_single_relation<T: Relation>(
    input: Value,
    label: &'static str,
) -> crate::Result<HasOne<T>> {
    match input {
        Value::Null => Ok(HasOne::default()),
        value => match T::load_relation(value.clone()) {
            // Current relation include encoding: a loaded single-relation slot
            // is the related model record directly.
            Ok(value) => Ok(HasOne {
                value: Some(Box::new(value)),
            }),
            Err(err) => match lazy_slot::decode(value, label, T::load_relation) {
                Ok(lazy_slot::LazySlot::Unloaded) => Ok(HasOne::default()),
                Ok(lazy_slot::LazySlot::Loaded(value)) => Ok(HasOne {
                    value: Some(Box::new(value)),
                }),
                Err(_) => Err(err),
            },
        },
    }
}

impl<T: Relation> HasOne<T> {
    /// Returns a reference to the loaded associated record.
    ///
    /// # Panics
    ///
    /// Panics if the association has not been loaded. Use [`try_get`] to
    /// handle the unloaded state without panicking.
    ///
    /// [`try_get`]: HasOne::try_get
    #[track_caller]
    pub fn get(&self) -> &T {
        self.value.as_ref().expect("association not loaded")
    }

    /// Returns a reference to the loaded associated record, or `None` if the
    /// association has not been loaded.
    ///
    /// This is the non-panicking counterpart to [`get`](HasOne::get). For an
    /// optional has-one (`HasOne<Option<T>>`), the inner `Option` reports
    /// whether the related row exists; the outer `Option` returned here
    /// reports whether the association was loaded.
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

    fn has_one_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
        FieldTy::HasOne(app::HasOne {
            target: <T::Model as Register>::id(),
            expr_ty: stmt::Type::Model(<T::Model as Register>::id()),
            kind: has_kind(pair, via),
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
