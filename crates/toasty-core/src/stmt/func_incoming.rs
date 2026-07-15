use super::{Expr, ExprFunc, Projection, Type};
use crate::schema::db::ColumnId;

/// A reference to a value proposed by an upsert's create branch.
#[derive(Clone, Debug, PartialEq)]
pub struct FuncIncoming {
    /// Field before lowering or column after lowering.
    pub target: IncomingTarget,

    /// Expression-level type of the proposed value.
    pub ty: Type,
}

/// The field or column referenced by [`FuncIncoming`].
#[derive(Clone, Debug, PartialEq)]
pub enum IncomingTarget {
    /// Application field before lowering.
    Field(Projection),

    /// Database column after lowering.
    Column(ColumnId),
}

impl FuncIncoming {
    /// Creates an incoming-value reference to an application field.
    pub fn field(field: usize, ty: Type) -> Self {
        Self {
            target: IncomingTarget::Field(Projection::from_index(field)),
            ty,
        }
    }
}

impl From<FuncIncoming> for Expr {
    fn from(value: FuncIncoming) -> Self {
        ExprFunc::Incoming(value).into()
    }
}
