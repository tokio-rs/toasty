use super::Load;
use crate::stmt::{self, Expr, List};

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

    /// The unwrapped type used for foreign key filter expressions.
    /// Primitives map to themselves. Wrappers like `Option<T>`, `Box<T>`,
    /// `Arc<T>`, and `Rc<T>` map to `T::Inner`.
    type Inner;

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

    /// Build a boolean filter expression comparing this field value against
    /// a target path. Used by generated belongs-to accessors to build FK
    /// lookups.
    ///
    /// For `T`, returns `target.eq(self)`. For `Option<T>`, returns
    /// `target.eq(inner)` when `Some`, or `false` when `None`.
    fn key_constraint<Origin>(&self, target: stmt::Path<Origin, Self::Inner>) -> Expr<bool>;
}

macro_rules! impl_field_primitive {
    ($ty:ty) => {
        impl Field for $ty {
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

impl<T: Field> Field for Option<T> {
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
}

impl<T: Field<Output = T>> Field for std::sync::Arc<T> {
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
}

impl<T: Field<Output = T>> Field for std::rc::Rc<T> {
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
}

impl<T: Field<Output = T>> Field for Box<T> {
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
}

#[cfg(feature = "rust_decimal")]
impl_field_primitive!(rust_decimal::Decimal);

#[cfg(feature = "bigdecimal")]
impl_field_primitive!(bigdecimal::BigDecimal);
