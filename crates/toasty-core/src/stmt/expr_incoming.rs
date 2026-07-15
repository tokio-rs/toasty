use super::{Expr, Projection, Type};
use crate::schema::db::ColumnId;

/// A reference to a value proposed by an upsert's create branch.
///
/// An update assignment uses this expression when it needs the incoming value
/// rather than the value already stored in the conflicting row. SQL serializers
/// map it to the backend's proposed-row syntax, such as PostgreSQL's
/// `EXCLUDED` relation.
#[derive(Clone, Debug, PartialEq)]
pub struct ExprIncoming {
    /// Field before lowering or column after lowering.
    pub target: IncomingTarget,

    /// Expression-level type of the proposed value.
    pub ty: Type,
}

/// The field or column referenced by [`ExprIncoming`].
#[derive(Clone, Debug, PartialEq)]
pub enum IncomingTarget {
    /// Application field before lowering.
    Field(Projection),

    /// Database column after lowering.
    Column(ColumnId),
}

impl ExprIncoming {
    /// Creates an incoming-value reference to an application field.
    pub fn field(field: usize, ty: Type) -> Self {
        Self {
            target: IncomingTarget::Field(Projection::from_index(field)),
            ty,
        }
    }
}

impl From<ExprIncoming> for Expr {
    fn from(value: ExprIncoming) -> Self {
        Self::Incoming(value)
    }
}
