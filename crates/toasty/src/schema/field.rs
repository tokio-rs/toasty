use std::{rc::Rc, sync::Arc};

use crate::{
    schema::{Load, ModelField},
    stmt::Path,
    Result,
};

use std::borrow::Cow;
use toasty_core::stmt;

pub trait Field: Sized {
    /// The type returned when accessing this field from a Fields struct.
    /// For primitives, this is Path<Origin, Self>.
    /// For embedded types, this is {Type}Fields<Origin>.
    type FieldAccessor<Origin>;

    /// The type of the update builder for this field.
    /// For embedded types, this is {Type}Update<'a>.
    /// For primitives, this will be {Type}Update<'a> once implemented.
    type UpdateBuilder<'a>;

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
}

/// Macro to generate Load, ModelField, and Field implementations for numeric types that use `try_into()`
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

                fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
                    *target = Self::load(value)?;
                    Ok(())
                }
            }

            impl ModelField for $ty {
                type Path<Origin> = Path<Origin, Self>;

                fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
                    path
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

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl ModelField for isize {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
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

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl ModelField for usize {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
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

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl ModelField for String {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl Field for String {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
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
            ty: stmt::Type::Bytes,
            storage_ty,
            serialize: None,
        })
    }
}

impl Field for Vec<u8> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = ();

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: ModelField> ModelField for Option<T> {
    type Path<Origin> = Path<Origin, Self>;
    const NULLABLE: bool = true;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl<T: Field> Field for Option<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T> Load for Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: Load<Output = T::Owned>,
{
    type Output = Self;

    fn ty() -> stmt::Type {
        <T::Owned as Load>::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T::Owned as Load>::load(value).map(Cow::Owned)
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl<T> ModelField for Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: ModelField<Output = T::Owned>,
{
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
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

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl ModelField for uuid::Uuid {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
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

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl ModelField for bool {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl Field for bool {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Load<Output = T>> Load for Arc<T> {
    type Output = Self;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Load>::load(value).map(Arc::new)
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl<T: ModelField<Output = T>> ModelField for Arc<T> {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl<T: Field> Field for Arc<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Load<Output = T>> Load for Rc<T> {
    type Output = Self;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Load>::load(value).map(Rc::new)
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl<T: ModelField<Output = T>> ModelField for Rc<T> {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
    }
}

impl<T: Field> Field for Rc<T> {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl<T: Load<Output = T>> Load for Box<T> {
    type Output = Self;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Load>::load(value).map(Box::new)
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl<T: ModelField<Output = T>> ModelField for Box<T> {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
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

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

#[cfg(feature = "rust_decimal")]
impl ModelField for rust_decimal::Decimal {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
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

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

#[cfg(feature = "bigdecimal")]
impl ModelField for bigdecimal::BigDecimal {
    type Path<Origin> = Path<Origin, Self>;

    fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
        path
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
