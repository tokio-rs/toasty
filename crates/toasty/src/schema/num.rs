use crate::{
    Result,
    schema::{Field, Load},
    stmt::{Expr, List, Path},
};

use toasty_core::stmt;

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

                fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
                    *target = Self::load(value)?;
                    Ok(())
                }
            }

            impl Field for $ty {
                type PathTarget = Self;
                type Path<Origin> = Path<Origin, Self>;
                type ListPath<Origin> = Path<Origin, List<Self>>;
                type Update<'a> = ();
                type Inner = Self;

                fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
                    path
                }

                fn new_list_path<Origin>(path: Path<Origin, List<Self>>) -> Self::ListPath<Origin> {
                    path
                }

                fn new_update<'a>(
                    _assignments: &'a mut toasty_core::stmt::Assignments,
                    _projection: toasty_core::stmt::Projection,
                ) -> Self::Update<'a> {
                }

                fn key_constraint<Origin>(
                    &self,
                    target: Path<Origin, Self::Inner>,
                ) -> Expr<bool> {
                    target.eq(self)
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
    f32 => F32,
    f64 => F64,
}
