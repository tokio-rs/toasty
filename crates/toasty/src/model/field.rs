use std::{rc::Rc, sync::Arc};

use crate::{stmt::Path, Result};

use std::borrow::Cow;
use toasty_core::stmt;

pub trait Field: Sized {
    /// Whether or not the type is nullable
    const NULLABLE: bool = false;

    /// The type returned when accessing this field from a Fields struct.
    /// For primitives, this is Path<Self>.
    /// For embedded types, this is {Type}Fields.
    type FieldAccessor;

    /// The type of the update builder for this field.
    /// For embedded types, this is {Type}Update<'a>.
    /// For primitives, this will be {Type}Update<'a> once implemented.
    type UpdateBuilder<'a>;

    fn ty() -> stmt::Type;

    fn load(value: stmt::Value) -> Result<Self>;

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
    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor;

    /// Build an update builder from a statement and projection.
    /// For primitives, this returns `()` (no builder).
    /// For embedded types, this is overridden to construct the {Type}Update builder.
    fn make_update_builder<'a>(
        _stmt: &'a mut stmt::Update,
        _projection: stmt::Projection,
    ) -> Self::UpdateBuilder<'a> {
        // Default implementation assumes UpdateBuilder = ()
        // Embedded types must override this method
        unsafe {
            // For (), this is safe. For other types, this would be UB,
            // but those types must override this method.
            std::mem::transmute_copy(&())
        }
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

/// Macro to generate Field implementations for numeric types that use `try_into()`
macro_rules! impl_field_numeric {
    ($($ty:ty => $stmt_ty:ident),* $(,)?) => {
        $(
            impl Field for $ty {
                type FieldAccessor = Path<Self>;
                type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

                fn ty() -> stmt::Type {
                    stmt::Type::$stmt_ty
                }

                fn load(value: stmt::Value) -> Result<Self> {
                    value.try_into()
                }

                fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
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
impl Field for isize {
    type FieldAccessor = Path<Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn ty() -> stmt::Type {
        stmt::Type::I64
    }

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl Field for usize {
    type FieldAccessor = Path<Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn ty() -> stmt::Type {
        stmt::Type::U64
    }

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl Field for String {
    type FieldAccessor = Path<Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn ty() -> stmt::Type {
        stmt::Type::String
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::String(v) => Ok(v),
            _ => Err(toasty_core::Error::type_conversion(value, "String")),
        }
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl Field for Vec<u8> {
    type FieldAccessor = Path<Self>;
    type UpdateBuilder<'a> = ();

    fn ty() -> stmt::Type {
        stmt::Type::Bytes
    }

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl<T: Field> Field for Option<T> {
    type FieldAccessor = Path<Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn ty() -> stmt::Type {
        T::ty()
    }
    const NULLABLE: bool = true;

    fn load(value: stmt::Value) -> Result<Self> {
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(T::load(value)?))
        }
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl<T> Field for Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: Field,
{
    type FieldAccessor = Path<Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn ty() -> stmt::Type {
        <T::Owned as Field>::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T::Owned as Field>::load(value).map(Cow::Owned)
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl Field for uuid::Uuid {
    type FieldAccessor = Path<Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn ty() -> stmt::Type {
        stmt::Type::Uuid
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::Uuid(v) => Ok(v),
            _ => Err(toasty_core::Error::type_conversion(value, "uuid::Uuid")),
        }
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl Field for bool {
    type FieldAccessor = Path<Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn ty() -> stmt::Type {
        stmt::Type::Bool
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::Bool(v) => Ok(v),
            _ => Err(toasty_core::Error::type_conversion(value, "bool")),
        }
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl<T: Field> Field for Arc<T> {
    type FieldAccessor = Path<Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Field>::load(value).map(Arc::new)
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl<T: Field> Field for Rc<T> {
    type FieldAccessor = Path<Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Field>::load(value).map(Rc::new)
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl<T: Field> Field for Box<T> {
    type FieldAccessor = Path<Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Field>::load(value).map(Box::new)
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

#[cfg(feature = "rust_decimal")]
impl Field for rust_decimal::Decimal {
    type FieldAccessor = Path<Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

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

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

#[cfg(feature = "bigdecimal")]
impl Field for bigdecimal::BigDecimal {
    type FieldAccessor = Path<Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

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

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}
