use super::*;

/// TODO: rename since this is also used in `Select`?
#[derive(Debug, Clone, PartialEq)]
pub enum Returning {
    // TODO: rename this `Model` as it returns the full model?
    Star,

    Changed,

    /// Return an expression
    Expr(Expr),
}

impl Returning {
    pub fn is_star(&self) -> bool {
        matches!(self, Returning::Star)
    }

    pub fn is_changed(&self) -> bool {
        matches!(self, Returning::Changed)
    }

    pub fn is_expr(&self) -> bool {
        matches!(self, Returning::Expr(_))
    }

    pub fn as_expr(&self) -> &Expr {
        match self {
            Returning::Expr(expr) => expr,
            _ => todo!(),
        }
    }

    pub fn as_expr_mut(&mut self) -> &mut Expr {
        match self {
            Returning::Expr(expr) => expr,
            _ => todo!(),
        }
    }
}

impl From<Expr> for Returning {
    fn from(value: Expr) -> Self {
        Returning::Expr(value)
    }
}
