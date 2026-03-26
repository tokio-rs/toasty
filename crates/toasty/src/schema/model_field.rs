use super::Load;
use crate::stmt;

/// Schema registration information for a field type.
///
/// This trait captures the information needed to register a field's type in the
/// app schema: its nullability and its [`FieldTy`](toasty_core::schema::app::FieldTy).
/// It is used by the `Register::schema()` implementation that the macro expands.
///
/// Separated from [`Field`](super::Field) so that schema registration does not
/// depend on runtime concerns like update builders or field accessors.
pub trait ModelField: Load {
    /// The type returned when accessing this field from a Fields struct.
    /// For primitives, this is Path<Origin, Self>.
    /// For embedded types, this is {Type}Fields<Origin>.
    type Path<Origin>;

    /// The type of the update builder for this field.
    /// For embedded types, this is {Type}Update<'a>.
    /// For primitives, this will be {Type}Update<'a> once implemented.
    type UpdateBuilder<'a>;

    /// Whether or not the type is nullable
    const NULLABLE: bool = false;

    /// Build a field path from a raw path.
    /// For primitives, returns the path as-is.
    /// For embedded types, wraps the path in a Fields struct.
    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin>
    where
        Self: Sized;

    /// Build an update builder from assignments and a projection.
    /// For primitives, this returns `()` (no builder).
    /// For embedded types, this is overridden to construct the {Type}Update builder.
    fn make_update_builder<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::UpdateBuilder<'a>;

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
}

macro_rules! impl_model_field_primitive {
    ($ty:ty) => {
        impl ModelField for $ty {
            type Path<Origin> = stmt::Path<Origin, Self>;
            type UpdateBuilder<'a> = ();

            fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
                path
            }

            fn make_update_builder<'a>(
                _assignments: &'a mut toasty_core::stmt::Assignments,
                _projection: toasty_core::stmt::Projection,
            ) -> Self::UpdateBuilder<'a> {
            }
        }
    };
}

impl_model_field_primitive!(String);
impl_model_field_primitive!(uuid::Uuid);
impl_model_field_primitive!(bool);
impl_model_field_primitive!(isize);
impl_model_field_primitive!(usize);

impl ModelField for Vec<u8> {
    type Path<Origin> = stmt::Path<Origin, Self>;
    type UpdateBuilder<'a> = ();

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn make_update_builder<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::UpdateBuilder<'a> {
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
}

impl<T: ModelField> ModelField for Option<T> {
    type Path<Origin> = stmt::Path<Origin, Self>;
    type UpdateBuilder<'a> = ();
    const NULLABLE: bool = true;

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn make_update_builder<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::UpdateBuilder<'a> {
    }
}

impl<T> ModelField for std::borrow::Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: ModelField<Output = T::Owned>,
{
    type Path<Origin> = stmt::Path<Origin, Self>;
    type UpdateBuilder<'a> = ();

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn make_update_builder<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::UpdateBuilder<'a> {
    }
}

impl<T: ModelField<Output = T>> ModelField for std::sync::Arc<T> {
    type Path<Origin> = stmt::Path<Origin, Self>;
    type UpdateBuilder<'a> = ();

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn make_update_builder<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::UpdateBuilder<'a> {
    }
}

impl<T: ModelField<Output = T>> ModelField for std::rc::Rc<T> {
    type Path<Origin> = stmt::Path<Origin, Self>;
    type UpdateBuilder<'a> = ();

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn make_update_builder<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::UpdateBuilder<'a> {
    }
}

impl<T: ModelField<Output = T>> ModelField for Box<T> {
    type Path<Origin> = stmt::Path<Origin, Self>;
    type UpdateBuilder<'a> = ();

    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }

    fn make_update_builder<'a>(
        _assignments: &'a mut toasty_core::stmt::Assignments,
        _projection: toasty_core::stmt::Projection,
    ) -> Self::UpdateBuilder<'a> {
    }
}

#[cfg(feature = "rust_decimal")]
impl_model_field_primitive!(rust_decimal::Decimal);

#[cfg(feature = "bigdecimal")]
impl_model_field_primitive!(bigdecimal::BigDecimal);
