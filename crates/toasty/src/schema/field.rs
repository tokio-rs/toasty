use super::Load;
use crate::stmt::{self, Expr, List};
use toasty_core::schema::app::ModelSet;

/// Schema and runtime information for a field type.
///
/// This trait captures the information needed to register a field's type in the
/// app schema (nullability, [`FieldTy`](toasty_core::schema::app::FieldTy)) as
/// well as runtime helpers for building field paths and update builders.
/// It is used by the `Register::schema()` implementation that the macro expands.
pub trait Field: Load {
    /// The expression-level type of this field.
    ///
    /// This drives both the field's path target (the second parameter of
    /// the underlying `Path`) **and** the `IntoExpr`/`Assign` bound on
    /// generated setters. For most types this is `Self`. For
    /// `Vec<T: Scalar>` it is [`List<T>`] so the field accessor returns a
    /// `Path<_, List<T>>` (giving access to `contains`, `is_superset`,
    /// `len`, …) and create/update setters accept any
    /// `impl IntoExpr<List<T>>` (Vec, slice, array literal, …).
    ///
    /// Decoupling this from `Self` lets the accessor macro construct the
    /// path with the right target type from the start, so `new_path` is
    /// identity for every primitive impl.
    type ExprTarget;

    /// The type returned when accessing this field from a Fields struct.
    /// For primitives, this is Path<Origin, Self>.
    /// For embedded types, this is {Type}Fields<Origin>.
    type Path<Origin>;

    /// The type returned when accessing this field from a list Fields struct.
    /// For primitives, this is Path<Origin, List<Self>>.
    /// For embedded types, this is {Type}ListFields<Origin>.
    type ListPath<Origin>;

    /// The type of the update builder for this field.
    /// For embedded types, this is {Type}Update<'a>.
    /// For primitives, this will be {Type}Update<'a> once implemented.
    type Update<'a>;

    /// The unwrapped type used for foreign key filter expressions.
    /// Primitives map to themselves. Wrappers like `Option<T>`, `Box<T>`,
    /// `Arc<T>`, and `Rc<T>` map to `T::Inner`.
    type Inner;

    /// Whether or not the type is nullable
    const NULLABLE: bool = false;

    /// Build a field path from a raw path of the field's
    /// [`Self::ExprTarget`].
    ///
    /// For primitives, returns the path as-is.
    /// For embedded types, wraps the path in a Fields struct.
    fn new_path<Origin>(path: stmt::Path<Origin, Self::ExprTarget>) -> Self::Path<Origin>
    where
        Self: Sized;

    /// Build a list field path from a raw path.
    /// For primitives, returns the path as-is.
    /// For embedded types, wraps the path in a ListFields struct.
    fn new_list_path<Origin>(path: stmt::Path<Origin, List<Self>>) -> Self::ListPath<Origin>
    where
        Self: Sized;

    /// Build an update builder from assignments and a projection.
    /// For primitives, this returns `()` (no builder).
    /// For embedded types, this is overridden to construct the {Type}Update builder.
    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a>;

    /// Returns the app-level field type for this primitive.
    /// Default implementation returns a Primitive field type.
    /// Embedded types override this to return Embedded field type.
    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        toasty_core::schema::app::FieldTy::Primitive(toasty_core::schema::app::FieldPrimitive {
            ty: Self::ty(),
            storage_ty,
            serialize: None,
        })
    }

    /// Build a boolean filter expression comparing this field value against
    /// a target path. Used by generated belongs-to accessors to build FK
    /// lookups.
    ///
    /// For `T`, returns `target.eq(self)`. For `Option<T>`, returns
    /// `target.eq(inner)` when `Some`, or `false` when `None`.
    fn key_constraint<Origin>(&self, target: stmt::Path<Origin, Self::Inner>) -> Expr<bool>;

    /// Register any models referenced by this field type into the given
    /// [`ModelSet`].
    ///
    /// The default implementation is a no-op, suitable for primitive types.
    /// Embedded types override this to call their own
    /// [`Register::register`](super::Register::register).
    fn register(_model_set: &mut ModelSet) {}
}

macro_rules! impl_field_primitive {
    ($ty:ty) => {
        impl Field for $ty {
            type ExprTarget = Self;
            type Path<Origin> = stmt::Path<Origin, Self>;
            type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
            type Update<'a> = ();
            type Inner = Self;

            fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
                path
            }

            fn new_list_path<Origin>(
                path: stmt::Path<Origin, List<Self>>,
            ) -> Self::ListPath<Origin> {
                path
            }

            fn new_update<'a>(
                _assignments: &'a mut toasty_core::stmt::Assignments,
                _projection: toasty_core::stmt::Projection,
            ) -> Self::Update<'a> {
            }

            fn key_constraint<Origin>(
                &self,
                target: stmt::Path<Origin, Self::Inner>,
            ) -> Expr<bool> {
                target.eq(self)
            }
        }
    };
}

impl_field_primitive!(String);
impl_field_primitive!(uuid::Uuid);
impl_field_primitive!(bool);
impl_field_primitive!(isize);
impl_field_primitive!(usize);

impl Field for Vec<u8> {
    type ExprTarget = Self;
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
    type Update<'a> = ();
    type Inner = Self;

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(path: stmt::Path<Origin, List<Self>>) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        toasty_core::schema::app::FieldTy::Primitive(toasty_core::schema::app::FieldPrimitive {
            ty: toasty_core::stmt::Type::Bytes,
            storage_ty,
            serialize: None,
        })
    }

    fn key_constraint<Origin>(&self, target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        target.eq(self)
    }
}

/// `Vec<T>` of a non-byte element type is a collection model field. The
/// bounds on `T` correspond to what the trait machinery does with elements:
///
/// - `Field<Output = T>` — the schema layer describes each element via
///   [`Load::ty()`](super::Load::ty); `Output = T` rules out wrappers like
///   `Box<U>` (whose `Output` is `Box<U>`) at the element position because the
///   load-side `Vec<T>: Load` impl is gated on `T: Load<Output = T>`.
/// - [`Scalar`] — opts the element type into the collection path. Keeps
///   `Vec<u8>` (bytes, not a collection) on the byte-array specialization
///   above and prevents nested collections like `Vec<Vec<U>>` until we
///   support them.
impl<T> Field for Vec<T>
where
    T: Field<Output = T> + Scalar,
{
    // Use the `List<T>` marker as the expression target so the field's
    // path is `Path<_, List<T>>` (giving `contains`, `is_superset`, `len`,
    // …) and the create/update setters bind through `IntoExpr<List<T>>`
    // (Vec, slice, array literal). `new_path` stays identity.
    type ExprTarget = List<T>;
    type Path<Origin> = stmt::Path<Origin, List<T>>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
    type Update<'a> = ();
    type Inner = Self;

    fn new_path<Origin>(path: stmt::Path<Origin, List<T>>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(path: stmt::Path<Origin, List<Self>>) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn key_constraint<Origin>(&self, _target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        // A foreign key cannot reference a `Vec<scalar>` field — collections
        // aren't valid key types. The trait method exists for every `Field`
        // impl, so we satisfy it with an explicit panic rather than wiring
        // up an `IntoExpr<Vec<T>>` bound that has no real use.
        unreachable!("Vec<T> fields cannot be used as foreign-key targets");
    }
}

/// Marks scalar (non-composite) types that are valid as the element type of a
/// model-level `Vec<T>` collection field. The trait is implemented for every
/// primitive `Field` (string, integers, uuid, …); `Vec<u8>` is intentionally
/// not a [`Scalar`] so that bytes keep their existing scalar storage and don't
/// get re-routed through the collection-field path.
pub trait Scalar {}

macro_rules! impl_scalar {
    ( $( $t:ty ),* $(,)? ) => {
        $( impl Scalar for $t {} )*
    };
}

impl_scalar!(
    String,
    bool,
    i8,
    i16,
    i32,
    i64,
    u16,
    u32,
    u64,
    isize,
    usize,
    f32,
    f64,
    uuid::Uuid
);

#[cfg(feature = "rust_decimal")]
impl Scalar for rust_decimal::Decimal {}

#[cfg(feature = "bigdecimal")]
impl Scalar for bigdecimal::BigDecimal {}

impl<T: Field> Field for Option<T> {
    type ExprTarget = Self;
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
    type Update<'a> = ();
    type Inner = T::Inner;
    const NULLABLE: bool = true;

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(path: stmt::Path<Origin, List<Self>>) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn key_constraint<Origin>(&self, target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        match self {
            Some(value) => T::key_constraint(value, target),
            None => Expr::from_value(toasty_core::stmt::Value::Bool(false)),
        }
    }

    fn register(model_set: &mut ModelSet) {
        T::register(model_set);
    }
}

impl<T: Field<Output = T>> Field for std::sync::Arc<T> {
    type ExprTarget = Self;
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
    type Update<'a> = ();
    type Inner = T::Inner;

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(path: stmt::Path<Origin, List<Self>>) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn key_constraint<Origin>(&self, target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        T::key_constraint(self, target)
    }

    fn register(model_set: &mut ModelSet) {
        T::register(model_set);
    }
}

impl<T: Field<Output = T>> Field for std::rc::Rc<T> {
    type ExprTarget = Self;
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
    type Update<'a> = ();
    type Inner = T::Inner;

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(path: stmt::Path<Origin, List<Self>>) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn key_constraint<Origin>(&self, target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        T::key_constraint(self, target)
    }

    fn register(model_set: &mut ModelSet) {
        T::register(model_set);
    }
}

impl<T: Field<Output = T>> Field for Box<T> {
    type ExprTarget = Self;
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
    type Update<'a> = ();
    type Inner = T::Inner;

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn new_list_path<Origin>(path: stmt::Path<Origin, List<Self>>) -> Self::ListPath<Origin> {
        path
    }

    fn new_update<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::Update<'a> {
    }

    fn key_constraint<Origin>(&self, target: stmt::Path<Origin, Self::Inner>) -> Expr<bool> {
        T::key_constraint(self, target)
    }

    fn register(model_set: &mut ModelSet) {
        T::register(model_set);
    }
}

#[cfg(feature = "rust_decimal")]
impl_field_primitive!(rust_decimal::Decimal);

#[cfg(feature = "bigdecimal")]
impl_field_primitive!(bigdecimal::BigDecimal);
