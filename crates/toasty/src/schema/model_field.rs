use super::Load;
use crate::stmt::Path;

/// Schema registration information for a field type.
///
/// This trait captures the information needed to register a field's type in the
/// app schema: its nullability and its [`FieldTy`](toasty_core::schema::app::FieldTy).
/// It is used by the `Register::schema()` implementation that the macro expands.
///
/// Separated from [`Field`](super::Field) so that schema registration does not
/// depend on runtime concerns like update builders or field accessors.
pub trait ModelField: Load + Sized {
    /// Whether or not the type is nullable
    const NULLABLE: bool = false;

    /// The type returned when accessing this field from a Fields struct.
    /// For primitives, this is Path<Origin, Self>.
    /// For embedded types, this is {Type}Fields<Origin>.
    type Path<Origin>;

    /// Build a path accessor from a path.
    /// For primitives, returns the path as-is.
    /// For embedded types, wraps the path in a Fields struct.
    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin>;

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

impl ModelField for String {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl ModelField for Vec<u8> {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
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
    const NULLABLE: bool = true;

    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl<T> ModelField for std::borrow::Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: ModelField<Output = T::Owned>,
{
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl ModelField for uuid::Uuid {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl ModelField for bool {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl<T: ModelField<Output = T>> ModelField for std::sync::Arc<T> {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl<T: ModelField<Output = T>> ModelField for std::rc::Rc<T> {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl<T: ModelField<Output = T>> ModelField for Box<T> {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl ModelField for isize {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl ModelField for usize {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

#[cfg(feature = "rust_decimal")]
impl ModelField for rust_decimal::Decimal {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

#[cfg(feature = "bigdecimal")]
impl ModelField for bigdecimal::BigDecimal {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}
