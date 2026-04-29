use super::{Field, Load};
use crate::stmt::{self, Expr};
use crate::{Executor, Result};
use toasty_core::schema::app::ModelSet;

use std::fmt;
use std::marker::PhantomData;

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

/// Builder returned by the per-field accessor on a model with a `#[deferred]`
/// primitive. Calling [`exec`](DeferredLoad::exec) issues a single-row read
/// against the database keyed on the model's primary key and returns the
/// deferred field's value.
///
/// `DeferredLoad` is constructed by generated code via [`new`](DeferredLoad::new)
/// and rewrites the statement's `RETURNING` clause to project just the deferred
/// column. The model record itself is never decoded, which keeps the loaded
/// state of nullable fields (`Deferred<Option<T>>`) unambiguous regardless of
/// whether the column value is `NULL`.
pub struct DeferredLoad<T> {
    stmt: toasty_core::stmt::Statement,
    _p: PhantomData<T>,
}

impl<T: Load<Output = T>> DeferredLoad<T> {
    /// Construct a new builder. Used by generated code.
    #[doc(hidden)]
    pub fn new(stmt: toasty_core::stmt::Statement) -> Self {
        Self {
            stmt,
            _p: PhantomData,
        }
    }

    /// Execute the load and return the deferred field's value.
    pub async fn exec(self, executor: &mut dyn Executor) -> Result<T> {
        let response = executor.exec_untyped(self.stmt).await?;
        let value = response.values.collect_as_value().await?;
        T::load(unwrap_single_column(value))
    }
}

/// Unwrap the value returned by a `SELECT col FROM tbl WHERE pk = ?` into the
/// scalar at the single column. Handles `Value::List([Value::Record([col])])`
/// (driver-shaped result), `Value::Record([col])` (single-row), and a bare
/// scalar.
fn unwrap_single_column(value: toasty_core::stmt::Value) -> toasty_core::stmt::Value {
    use toasty_core::stmt::Value;
    match value {
        Value::List(items) => {
            let mut iter = items.into_iter();
            match iter.next() {
                Some(first) => unwrap_single_column(first),
                None => Value::Null,
            }
        }
        Value::Record(record) if record.fields.len() == 1 => {
            record.fields.into_iter().next().unwrap()
        }
        v => v,
    }
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
        // A deferred field is loaded as Null when the column was excluded from
        // the default projection — the unloaded state. (Phase 1: only the
        // unloaded path is reachable through the model load. `.include()` is
        // not yet supported.)
        match value {
            toasty_core::stmt::Value::Null => Ok(Self { value: None }),
            value => Ok(Self {
                value: Some(T::load(value)?),
            }),
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
