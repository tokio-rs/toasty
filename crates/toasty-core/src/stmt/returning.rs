use super::*;

/// TODO: rename since this is also used in `Select`?
#[derive(Debug, Clone)]
pub enum Returning {
    // TODO: rename this `Model` as it returns the full model?
    Star,

    Changed,

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
