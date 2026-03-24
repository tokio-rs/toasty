use std::{rc::Rc, sync::Arc};

use crate::{schema::Load, stmt::Path, Result};

use std::borrow::Cow;
use toasty_core::stmt;

pub trait Field: Sized + Load<Output = Self> {
    /// Whether or not the type is nullable
    const NULLABLE: bool = false;

    /// The type returned when accessing this field from a Fields struct.
    /// For primitives, this is Path<Origin, Self>.
    /// For embedded types, this is {Type}Fields<Origin>.
    type FieldAccessor<Origin>;

    /// The type of the update builder for this field.
    /// For embedded types, this is {Type}Update<'a>.
    /// For primitives, this will be {Type}Update<'a> once implemented.
    type UpdateBuilder<'a>;

    /// Reload the value in-place from a value returned by the database.
    ///
    /// The value may be a `SparseRecord` for partial embedded updates, in which
    /// case only the specified fields should be updated. Embedded types must
    /// override this method to handle partial updates correctly.
    fn reload(&mut self, value: stmt::Value) -> Result<()> {
        *self = Self::load(value)?;
        Ok(())
    }

    /// Build a field accessor from a path.
    /// For primitives, returns the path as-is.
    /// For embedded types, wraps the path in a Fields struct.
    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin>;

    /// Build an update builder from assignments and a projection.
    /// For primitives, this returns `()` (no builder).
    /// For embedded types, this is overridden to construct the {Type}Update builder.
    fn make_update_builder<'a>(
        _assignments: &'a mut stmt::Assignments,
        _projection: stmt::Projection,
    ) -> Self::UpdateBuilder<'a> {
        // Embedded types must override this method.
        // For primitive types (where UpdateBuilder = ()), this is never called.
        panic!("make_update_builder must be overridden")
    }

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

/// Macro to generate Load and Field implementations for numeric types that use `try_into()`
macro_rules! impl_field_numeric {
    ($($ty:ty => $stmt_ty:ident),* $(,)?) => {
        $(
            impl Load for $ty {
                type Output = Self;

                fn ty() -> stmt::Type {
                    stmt::Type::$stmt_ty
                }

                fn load(value: stmt::Value) -> Result<Self> {
                    value.try_into()
                }
            }

            impl Field for $ty {
                type FieldAccessor<Origin> = Path<Origin, Self>;
                type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

                fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
                    path
                }
            }
        )*
    };
}

// Generate implementations for all numeric types
impl_field_numeric! {
    i8 => I8,
    i16 => I16,
    i32 => I32,
    i64 => I64,
    u8 => U8,
    u16 => U16,
    u32 => U32,
    u64 => U64,
}

// Pointer-sized integers map to fixed-size types internally
impl Load for isize {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::I64
    }

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
}

impl Field for isize {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Load for usize {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::U64
    }

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
}

impl Field for usize {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Load for String {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::String
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::String(v) => Ok(v),
            _ => Err(toasty_core::Error::type_conversion(value, "String")),
        }
    }
}

impl Field for String {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Field for Vec<u8> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = ();

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }

    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        toasty_core::schema::app::FieldTy::Primitive(toasty_core::schema::app::FieldPrimitive {
            ty: stmt::Type::Bytes,
            storage_ty,
            serialize: None,
        })
    }
}

impl<T: Field> Field for Option<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    const NULLABLE: bool = true;

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T> Load for Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: Field,
{
    type Output = Self;

    fn ty() -> stmt::Type {
        <T::Owned as Load>::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T::Owned as Load>::load(value).map(Cow::Owned)
    }
}

impl<T> Field for Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: Field,
{
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Load for uuid::Uuid {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::Uuid
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::Uuid(v) => Ok(v),
            _ => Err(toasty_core::Error::type_conversion(value, "uuid::Uuid")),
        }
    }
}

impl Field for uuid::Uuid {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Load for bool {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::Bool
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::Bool(v) => Ok(v),
            _ => Err(toasty_core::Error::type_conversion(value, "bool")),
        }
    }
}

impl Field for bool {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Field> Load for Arc<T> {
    type Output = Self;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Load>::load(value).map(Arc::new)
    }
}

impl<T: Field> Field for Arc<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Field> Load for Rc<T> {
    type Output = Self;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Load>::load(value).map(Rc::new)
    }
}

impl<T: Field> Field for Rc<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Field> Load for Box<T> {
    type Output = Self;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Load>::load(value).map(Box::new)
    }
}

impl<T: Field> Field for Box<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

#[cfg(feature = "rust_decimal")]
impl Load for rust_decimal::Decimal {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::Decimal
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::Decimal(v) => Ok(v),
            _ => Err(toasty_core::Error::type_conversion(
                value,
                "rust_decimal::Decimal",
            )),
        }
    }
}

#[cfg(feature = "rust_decimal")]
impl Field for rust_decimal::Decimal {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

#[cfg(feature = "bigdecimal")]
impl Load for bigdecimal::BigDecimal {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::BigDecimal
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::BigDecimal(v) => Ok(v),
            _ => Err(toasty_core::Error::type_conversion(
                value,
                "bigdecimal::BigDecimal",
            )),
        }
    }
}

#[cfg(feature = "bigdecimal")]
impl Field for bigdecimal::BigDecimal {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}
