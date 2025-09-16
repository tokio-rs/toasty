use super::{Expr, Path};
use crate::stmt;

/// TODO: rename since this is also used in `Select`?
#[derive(Debug, Clone)]
pub enum Returning {
    /// Return all fields (SQL-level only)
    Star,

    /// Return only changed fields
    Changed,

    /// Return a model with optional associations (app-level)
    Model {
        /// Associations to include
        include: Vec<Path>,
    },

    /// Return an expression
    Expr(Expr),
}

impl Returning {
    pub fn is_star(&self) -> bool {
        matches!(self, Self::Star)
    }

    pub fn is_changed(&self) -> bool {
        matches!(self, Self::Changed)
    }

    pub fn is_model(&self) -> bool {
        matches!(self, Self::Model { .. })
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

    pub fn as_model(&self) -> &Vec<Path> {
        match self {
            Self::Model { include } => include,
            _ => todo!("self={self:#?}"),
        }
    }

    pub fn as_model_mut(&mut self) -> &mut Vec<Path> {
        match self {
            Self::Model { include } => include,
            _ => todo!("self={self:#?}"),
        }
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
