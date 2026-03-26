use crate::{
    schema::{Load, ModelField, Scope},
    stmt::Path,
    Result,
};

use toasty_core::stmt;

/// Macro to generate Load, ModelField, and Scope implementations for numeric types that use `try_into()`
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

            impl Scope for $ty {
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
