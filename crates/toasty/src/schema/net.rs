use super::{Field, Load};
use crate::stmt::{Expr, IpCidr, IpInet, List, MacAddr6, MacAddr8, Path};
use toasty_core::{
    Result,
    stmt::{Type, Value},
};

macro_rules! impl_net_field {
    ($ty:ty, $name:ident, $lit:literal) => {
        impl Load for $ty {
            type Output = Self;

            fn ty() -> Type {
                Type::$name
            }

            fn load(value: Value) -> Result<Self> {
                match value {
                    Value::$name(v) => Ok(v),
                    _ => Err(toasty_core::Error::type_conversion(value, $lit)),
                }
            }

            fn reload(target: &mut Self, value: Value) -> Result<()> {
                *target = Self::load(value)?;
                Ok(())
            }
        }

        impl Field for $ty {
            type ExprTarget = Self;
            type Path<Origin> = Path<Origin, Self>;
            type ListPath<Origin> = Path<Origin, List<Self::ExprTarget>>;
            type Update<'a> = ();
            type Inner = Self;

            fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
                path
            }

            fn new_list_path<Origin>(
                path: Path<Origin, List<Self::ExprTarget>>,
            ) -> Self::ListPath<Origin> {
                path
            }

            fn new_update<'a>(
                _assignments: &'a mut toasty_core::stmt::Assignments,
                _projection: toasty_core::stmt::Projection,
            ) -> Self::Update<'a> {
            }

            fn key_constraint<Origin>(&self, target: Path<Origin, Self::Inner>) -> Expr<bool> {
                target.eq(self)
            }
        }
    };
}

impl_net_field!(IpCidr, Cidr, "cidr::IpCidr");
impl_net_field!(IpInet, Inet, "cidr::IpInet");
impl_net_field!(MacAddr6, MacAddr, "macaddr::MacAddr6");
impl_net_field!(MacAddr8, MacAddr8, "macaddr::MacAddr8");
