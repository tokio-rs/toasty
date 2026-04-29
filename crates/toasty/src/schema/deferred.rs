use super::{Field, Load};
use crate::Statement;
use crate::stmt::{self, Expr, IntoStatement};
use toasty_core::schema::app::ModelSet;

use std::fmt;

/// A field whose value is not loaded by default.
///
/// `Deferred<T>` wraps a `T` whose underlying column is excluded from default
/// queries. After a normal load the value is unloaded and accessing it via
/// [`get`](Deferred::get) panics. Fetch the value with the per-field
/// `.exec()` accessor on the record.
///
/// `Deferred<Option<T>>` is supported when the column is nullable.
pub struct Deferred<T> {
    value: Option<T>,
}

/// Marker trait identifying a deferred-load field wrapper, exposing the wrapped
/// value type via [`Inner`](Defer::Inner).
///
/// Generated code references `<F as Defer>::Inner` to recover the user-facing
/// value type for fields annotated with `#[deferred]`. The trait is implemented
/// only for [`Deferred<T>`], so applying `#[deferred]` to a field whose type is
/// not `Deferred<T>` (after type aliases are resolved) fails to compile.
#[diagnostic::on_unimplemented(
    message = "`#[deferred]` requires the field to be wrapped in `Deferred<T>`",
    label = "expected `Deferred<T>`, found `{Self}`"
)]
pub trait Defer {
    /// The wrapped value type.
    type Inner;
}

impl<T> Defer for Deferred<T> {
    type Inner = T;
}

impl<T> Deferred<T> {
    /// Returns `true` if the field has not been loaded.
    pub fn is_unloaded(&self) -> bool {
        self.value.is_none()
    }

    /// Returns a reference to the loaded value.
    ///
    /// # Panics
    ///
    /// Panics if the field has not been loaded.
    #[track_caller]
    pub fn get(&self) -> &T {
        self.value.as_ref().expect("deferred field not loaded")
    }

    /// Clears the loaded value, returning the field to the unloaded state.
    pub fn unload(&mut self) {
        self.value = None;
    }

    /// Consumes this `Deferred<T>` and returns the loaded value.
    ///
    /// # Panics
    ///
    /// Panics if the field has not been loaded.
    #[track_caller]
    pub fn into_inner(self) -> T {
        self.value.expect("deferred field not loaded")
    }
}

/// Build a `Statement<T>` from a PK-filtered single-row query and the
/// model-field index of the deferred field. Rewrites the statement's
/// `RETURNING` clause to project just that column.
///
/// Used by generated code for the per-field accessor on `#[deferred]`
/// primitives. The model record itself is never decoded, which keeps the
/// loaded state of nullable fields (`Deferred<Option<T>>`) unambiguous
/// regardless of whether the column value is `NULL`.
#[doc(hidden)]
pub fn build_deferred_load<T, S: IntoStatement>(stmt: S, field_index: usize) -> Statement<T> {
    let mut untyped = stmt.into_statement().into_untyped();
    *untyped.returning_mut_unwrap() = toasty_core::stmt::Returning::Project(
        toasty_core::stmt::Expr::Reference(toasty_core::stmt::ExprReference::Field {
            nesting: 0,
            index: field_index,
        }),
    );
    Statement::from_untyped_stmt(untyped)
}

impl<T> Default for Deferred<T> {
    fn default() -> Self {
        Self { value: None }
    }
}

impl<T: Load<Output = T>> Load for Deferred<T> {
    type Output = Self;

    fn ty() -> toasty_core::stmt::Type {
        T::ty()
    }

    fn load(value: toasty_core::stmt::Value) -> crate::Result<Self> {
        // The lowering wraps loaded deferred slots in a 1-element record and
        // emits a bare Null for unloaded slots, so the two states are
        // distinguishable even when the inner column value is NULL (i.e. the
        // `Deferred<Option<T>>` case).
        match value {
            toasty_core::stmt::Value::Null => Ok(Self { value: None }),
            toasty_core::stmt::Value::Record(record) if record.fields.len() == 1 => {
                let mut iter = record.fields.into_iter();
                Ok(Self {
                    value: Some(T::load(iter.next().unwrap())?),
                })
            }
            value => Err(toasty_core::Error::from_args(format_args!(
                "deferred field decoder expected Null or single-field Record, got {value:?}"
            ))),
        }
    }

    fn reload(_target: &mut Self, _value: toasty_core::stmt::Value) -> crate::Result<()> {
        // An update does not change a deferred field's loaded state. The
        // in-memory copy (loaded or unloaded) is left as-is.
        Ok(())
    }
}

impl<T: Field<Output = T>> Field for Deferred<T> {
    type Path<Origin> = T::Path<Origin>;
    type ListPath<Origin> = T::ListPath<Origin>;
    type Update<'a> = T::Update<'a>;
    type Inner = T::Inner;
    const NULLABLE: bool = T::NULLABLE;

    fn new_path<Origin>(_path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        // Deferred fields use a generated accessor that emits a path on the
        // inner type T directly (see `expand_primitive_field_method`'s
        // deferred arm). This impl is unreachable through normal codegen.
        unreachable!("Deferred::new_path should not be called directly")
    }

    fn new_list_path<Origin>(
        _path: stmt::Path<Origin, stmt::List<Self>>,
    ) -> Self::ListPath<Origin> {
        unreachable!("Deferred::new_list_path should not be called directly")
    }

    fn new_update<'a>(
        assignments: &'a mut toasty_core::stmt::Assignments,
        projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
        T::new_update(assignments, projection)
    }

    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        T::field_ty(storage_ty)
    }

    fn key_constraint<Origin>(&self, _target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        // Foreign keys cannot reference a deferred field.
        unreachable!("Deferred fields cannot be used as foreign keys")
    }

    fn register(model_set: &mut ModelSet) {
        T::register(model_set);
    }
}

impl<T: fmt::Debug> fmt::Debug for Deferred<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.value {
            Some(value) => value.fmt(f),
            None => write!(f, "<not loaded>"),
        }
    }
}

#[cfg(feature = "serde")]
impl<T: serde_core::Serialize> serde_core::Serialize for Deferred<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde_core::Serializer,
    {
        self.value.serialize(serializer)
    }
}
