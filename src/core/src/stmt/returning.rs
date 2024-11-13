use super::*;

/// TODO: rename since this is also used in `Select`?
#[derive(Debug, Clone, PartialEq)]
pub enum Returning<'stmt> {
    // TODO: rename this `Model` as it returns the full model?
    Star,

    Changed,

    /// Return an expression
    Expr(Expr<'stmt>),
}

impl<'stmt> Returning<'stmt> {
    pub fn is_star(&self) -> bool {
        matches!(self, Returning::Star)
    }

    pub fn is_changed(&self) -> bool {
        matches!(self, Returning::Changed)
    }

    pub fn is_expr(&self) -> bool {
        matches!(self, Returning::Expr(_))
    }

    pub fn as_expr(&self) -> &Expr<'stmt> {
        match self {
            Returning::Expr(expr) => expr,
            _ => todo!(),
        }
    }

    pub fn as_expr_mut(&mut self) -> &mut Expr<'stmt> {
        match self {
            Returning::Expr(expr) => expr,
            _ => todo!(),
        }
    }
}

impl<'stmt> From<Expr<'stmt>> for Returning<'stmt> {
    fn from(value: Expr<'stmt>) -> Self {
        Returning::Expr(value)
    }
}
