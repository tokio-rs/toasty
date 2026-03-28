use super::Load;
use crate::stmt::{self, List};

/// Schema and runtime information for a field type.
///
/// This trait captures the information needed to register a field's type in the
/// app schema (nullability, [`FieldTy`](toasty_core::schema::app::FieldTy)) as
/// well as runtime helpers for building field paths and update builders.
/// It is used by the `Register::schema()` implementation that the macro expands.
pub trait Field: Load {
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

    /// Whether or not the type is nullable
    const NULLABLE: bool = false;

    /// Build a field path from a raw path.
    /// For primitives, returns the path as-is.
    /// For embedded types, wraps the path in a Fields struct.
    fn new_path<Origin>(path: stmt::Path<Origin, Self>) -> Self::Path<Origin>
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
}

macro_rules! impl_field_primitive {
    ($ty:ty) => {
        impl Field for $ty {
            type Path<Origin> = stmt::Path<Origin, Self>;
            type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
            type Update<'a> = ();

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
        }
    };
}

impl_field_primitive!(String);
impl_field_primitive!(uuid::Uuid);
impl_field_primitive!(bool);
impl_field_primitive!(isize);
impl_field_primitive!(usize);

impl Field for Vec<u8> {
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
    type Update<'a> = ();

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
}

impl<T: Field> Field for Option<T> {
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
    type Update<'a> = ();
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
}

impl<T> Field for std::borrow::Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: Field<Output = T::Owned>,
{
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
    type Update<'a> = ();

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
}

impl<T: Field<Output = T>> Field for std::sync::Arc<T> {
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
    type Update<'a> = ();

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
}

impl<T: Field<Output = T>> Field for std::rc::Rc<T> {
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
    type Update<'a> = ();

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
}

impl<T: Field<Output = T>> Field for Box<T> {
    type Path<Origin> = stmt::Path<Origin, Self>;
    type ListPath<Origin> = stmt::Path<Origin, List<Self>>;
    type Update<'a> = ();

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
}

#[cfg(feature = "rust_decimal")]
impl_field_primitive!(rust_decimal::Decimal);

#[cfg(feature = "bigdecimal")]
impl_field_primitive!(bigdecimal::BigDecimal);
