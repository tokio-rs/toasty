use super::{Expr, Path};
use crate::stmt::{self, Node, Value};

/// TODO: rename since this is also used in `Select`?
#[derive(Debug, Clone, PartialEq)]
pub enum Returning {
    /// Return the full model with specified includes
    Model {
        include: Vec<Path>,
    },

    Changed,

    /// Return an expression
    Expr(Expr),
}

impl Returning {
    pub fn from_expr_iter<T>(items: impl IntoIterator<Item = T>) -> Self
    where
        T: Into<Expr>,
    {
        Returning::Expr(Expr::record(items))
    }

    pub fn is_model(&self) -> bool {
        matches!(self, Self::Model { .. })
    }

    pub fn as_model_includes(&self) -> &[Path] {
        match self {
            Self::Model { include } => include,
            _ => &[],
        }
    }

    pub fn as_model_includes_mut(&mut self) -> &mut Vec<Path> {
        match self {
            Self::Model { include } => include,
            _ => panic!("not a Model variant"),
        }
    }

    pub fn is_changed(&self) -> bool {
        matches!(self, Self::Changed)
    }

    pub fn is_expr(&self) -> bool {
        matches!(self, Self::Expr(_))
    }

    pub fn as_expr(&self) -> &Expr {
        match self {
            Self::Expr(expr) => expr,
            _ => todo!("self={self:#?}"),
        }
    }

    pub fn as_expr_mut(&mut self) -> &mut Expr {
        match self {
            Self::Expr(expr) => expr,
            _ => todo!(),
        }
    }

    pub fn into_expr(self) -> Expr {
        match self {
            Self::Expr(expr) => expr,
            _ => todo!("self={self:#?}"),
        }
    }
}

impl<T> From<T> for Returning
where
    Value: From<T>,
{
    fn from(value: T) -> Self {
        Returning::Expr(Value::from(value).into())
    }
}

impl From<Expr> for Returning {
    fn from(value: Expr) -> Self {
        Self::Expr(value)
    }
}

impl From<Vec<Expr>> for Returning {
    fn from(value: Vec<Expr>) -> Self {
        stmt::Returning::Expr(stmt::Expr::record_from_vec(value))
    }
}

impl Node for Returning {
    fn visit<V: stmt::Visit>(&self, mut visit: V)
    where
        Self: Sized,
    {
        visit.visit_returning(self);
    }

    fn visit_mut<V: stmt::VisitMut>(&mut self, mut visit: V) {
        visit.visit_returning_mut(self);
    }
}
