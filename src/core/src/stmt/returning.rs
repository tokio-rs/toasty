use super::*;

/// TODO: rename since this is also used in `Select`?
#[derive(Debug, Clone, PartialEq)]
pub enum Returning<'stmt> {
    // TODO: rename this `Model` as it returns the full model?
    Star,

    /// Return an expression
    Expr(Expr<'stmt>),
}

impl<'stmt> Returning<'stmt> {
    pub fn is_star(&self) -> bool {
        matches!(self, Returning::Star)
    }
}
