use std::{rc::Rc, sync::Arc};

use crate::{
    stmt::{Id, Path},
    Model, Result,
};

use std::borrow::Cow;
use toasty_core::{
    schema::app::{AutoStrategy, UuidVersion},
    stmt,
};

pub trait Primitive: Sized {
    /// Whether or not the type is nullable
    const NULLABLE: bool = false;

    /// The type returned when accessing this field from a Fields struct.
    /// For primitives, this is Path<Self>.
    /// For embedded types, this is {Type}Fields.
    type FieldAccessor;

    fn ty() -> stmt::Type;

    fn load(value: stmt::Value) -> Result<Self>;

    /// Build a field accessor from a path.
    /// For primitives, returns the path as-is.
    /// For embedded types, wraps the path in a Fields struct.
    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor;

    /// Returns the app-level field type for this primitive.
    /// Default implementation returns a Primitive field type.
    /// Embedded types override this to return Embedded field type.
    fn field_ty(
        storage_ty: Option<toasty_core::schema::db::Type>,
    ) -> toasty_core::schema::app::FieldTy {
        toasty_core::schema::app::FieldTy::Primitive(toasty_core::schema::app::FieldPrimitive {
            ty: Self::ty(),
            storage_ty,
        })
    }
}

#[diagnostic::on_unimplemented(
    message = "Toasty cannot automatically set values for type `{Self}`",
    label = "Toasty cannot automatically set values for this field",
    note = "Is the field annotated with #[auto]?"
)]
pub trait Auto: Primitive {
    const STRATEGY: AutoStrategy;
}

/// Macro to generate Primitive implementations for numeric types that use `try_into()`
macro_rules! impl_primitive_numeric {
    ($($ty:ty => $stmt_ty:ident),* $(,)?) => {
        $(
            impl Primitive for $ty {
                type FieldAccessor = Path<Self>;

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

            impl Auto for $ty {
                const STRATEGY: AutoStrategy = AutoStrategy::Increment;
            }
        )*
    };
}

// Generate implementations for all numeric types
impl_primitive_numeric! {
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
impl Primitive for isize {
    type FieldAccessor = Path<Self>;

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

impl Auto for isize {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Primitive for usize {
    type FieldAccessor = Path<Self>;

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

impl Auto for usize {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Primitive for String {
    type FieldAccessor = Path<Self>;

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

impl<T: Model> Primitive for Id<T> {
    type FieldAccessor = Path<Self>;

    fn ty() -> stmt::Type {
        stmt::Type::Id(T::id())
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::Id(v) => Ok(Self::from_untyped(v)),
            _ => Err(toasty_core::Error::type_conversion(value, "Id")),
        }
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl<T: Model> Auto for Id<T> {
    const STRATEGY: AutoStrategy = AutoStrategy::Id;
}

impl<T: Primitive> Primitive for Option<T> {
    type FieldAccessor = Path<Self>;

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

impl<T> Primitive for Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: Primitive,
{
    type FieldAccessor = Path<Self>;

    fn ty() -> stmt::Type {
        <T::Owned as Primitive>::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T::Owned as Primitive>::load(value).map(Cow::Owned)
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl Primitive for uuid::Uuid {
    type FieldAccessor = Path<Self>;

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

impl Auto for uuid::Uuid {
    const STRATEGY: AutoStrategy = AutoStrategy::Uuid(UuidVersion::V7);
}

impl Primitive for bool {
    type FieldAccessor = Path<Self>;

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

impl<T: Primitive> Primitive for Arc<T> {
    type FieldAccessor = Path<Self>;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Primitive>::load(value).map(Arc::new)
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl<T: Primitive> Primitive for Rc<T> {
    type FieldAccessor = Path<Self>;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Primitive>::load(value).map(Rc::new)
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

impl<T: Primitive> Primitive for Box<T> {
    type FieldAccessor = Path<Self>;

    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Primitive>::load(value).map(Box::new)
    }

    fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
        path
    }
}

#[cfg(feature = "rust_decimal")]
impl Primitive for rust_decimal::Decimal {
    type FieldAccessor = Path<Self>;

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
impl Primitive for bigdecimal::BigDecimal {
    type FieldAccessor = Path<Self>;

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
