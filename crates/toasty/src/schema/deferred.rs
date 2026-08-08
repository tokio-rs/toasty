use super::{Field, Load, lazy_slot};
use crate::stmt::{self, Expr, IntoExpr};
use toasty_core::schema::app::ModelSet;

use std::fmt;

/// A field whose value is not loaded by default.
///
/// `Deferred<T>` wraps a `T` whose underlying column is excluded from default
/// queries. After a normal load the value is unloaded and accessing it via
/// [`get`](Deferred::get) panics. Use `.include()` on the query to load the
/// value.
///
/// `Deferred<Option<T>>` is supported when the column is nullable.
///
/// # Serde
///
/// With the `serde` feature a loaded `Deferred<T>` serializes transparently as
/// its inner `T`, and any present value deserializes back to the loaded state.
/// The unloaded state has no transparent encoding of its own — skip it on the
/// way out and default it on the way in:
///
/// ```ignore
/// #[serde(skip_serializing_if = "Deferred::is_unloaded", default)]
/// notes: Deferred<Option<String>>,
/// ```
///
/// Serializing an unloaded field without `skip_serializing_if` emits `null`,
/// which does not round-trip (it reads back as loaded), so the annotation is
/// expected on every deferred field.
#[derive(Clone)]
pub struct Deferred<T> {
    value: Option<Box<T>>,
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
        *self.value.expect("deferred field not loaded")
    }
}

impl<T> Default for Deferred<T> {
    fn default() -> Self {
        Self { value: None }
    }
}

impl<T> From<T> for Deferred<T> {
    /// Constructs a loaded `Deferred<T>` from a value.
    ///
    /// Used in struct literals for `#[derive(Embed)]` types that contain
    /// deferred sub-fields, where the user supplies the inner value
    /// directly: `Metadata { author, notes: "...".into() }`.
    fn from(value: T) -> Self {
        Self {
            value: Some(Box::new(value)),
        }
    }
}

/// Forwards the inner value's expression encoding so a `Deferred<T>` field
/// can be spliced into any site that accepts a `T`.
///
/// `Deferred<T>` is a load-state wrapper, not a value type, so the produced
/// expression is `Expr<T>` (the value), never `Expr<Deferred<T>>`. Panics
/// with the standard "deferred field not loaded" error if the value is in
/// the unloaded state — matching `.get()` / `.into_inner()`.
impl<T: IntoExpr<T>> IntoExpr<T> for Deferred<T> {
    fn into_expr(self) -> Expr<T> {
        self.into_inner().into_expr()
    }

    fn by_ref(&self) -> Expr<T> {
        self.get().by_ref()
    }
}

impl<T: IntoExpr<T>> IntoExpr<T> for &Deferred<T> {
    fn into_expr(self) -> Expr<T> {
        self.get().by_ref()
    }

    fn by_ref(&self) -> Expr<T> {
        self.get().by_ref()
    }
}

impl<T: Load<Output = T>> Load for Deferred<T> {
    type Output = Self;

    fn ty() -> toasty_core::stmt::Type {
        T::ty()
    }

    fn load(value: toasty_core::stmt::Value) -> crate::Result<Self> {
        // The lowering wraps a loaded deferred field in a 1-element record
        // and emits a bare Null when unloaded, so the two states are
        // distinguishable even when the inner value is NULL (i.e. the
        // `Deferred<Option<T>>` case).
        match lazy_slot::decode(value, "deferred field", T::load)? {
            lazy_slot::LazySlot::Unloaded => Ok(Self { value: None }),
            lazy_slot::LazySlot::Loaded(value) => Ok(Self {
                value: Some(Box::new(value)),
            }),
        }
    }

    fn reload(target: &mut Self, value: toasty_core::stmt::Value) -> crate::Result<()> {
        // The caller already supplied the value as part of the update, so the
        // field becomes loaded regardless of its prior state — no follow-up
        // fetch is needed to read what was just written.
        //
        // Updates send the assigned value back unwrapped, unlike the SELECT
        // lowering which wraps a loaded deferred field in a 1-element record
        // to distinguish it from the unloaded case.
        target.value = Some(Box::new(T::load(value)?));
        Ok(())
    }
}

impl<T: Field> Field for Deferred<T> {
    const REQUIRES_EXPLICIT_COLUMN_TYPE: bool = T::REQUIRES_EXPLICIT_COLUMN_TYPE;

    type ExprTarget = T::ExprTarget;
    type Path<Origin> = T::Path<Origin>;
    type ListPath<Origin> = T::ListPath<Origin>;
    type Update<'a> = T::Update<'a>;
    type Inner = T::Inner;
    const NULLABLE: bool = T::NULLABLE;
    const DEFERRED: bool = true;

    fn new_path<Origin>(path: stmt::Path<Origin, T::ExprTarget>) -> Self::Path<Origin> {
        T::new_path(path)
    }

    fn new_list_path<Origin>(
        path: stmt::Path<Origin, stmt::List<Self::ExprTarget>>,
    ) -> Self::ListPath<Origin> {
        T::new_list_path(path)
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

/// Serializes transparently: a loaded `Deferred<T>` encodes exactly as the
/// inner `T`, an unloaded one as `null`.
///
/// The unloaded `null` is not a distinct marker — a loaded `Deferred<Option<T>>`
/// holding `None` also encodes as `null` — so unloaded fields are meant to be
/// skipped rather than serialized. See the type-level docs.
#[cfg(feature = "serde")]
impl<T: serde_core::Serialize> serde_core::Serialize for Deferred<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde_core::Serializer,
    {
        self.value.serialize(serializer)
    }
}

/// Deserializes any present value — including `null` — as loaded, mirroring the
/// transparent [`Serialize`](serde_core::Serialize) impl above. An absent field
/// is left unloaded, but only via `#[serde(default)]`; see the type-level docs.
#[cfg(feature = "serde")]
impl<'de, T: serde_core::Deserialize<'de>> serde_core::Deserialize<'de> for Deferred<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde_core::Deserializer<'de>,
    {
        Ok(Self::from(T::deserialize(deserializer)?))
    }
}
