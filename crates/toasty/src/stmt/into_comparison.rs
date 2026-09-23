use super::{Expr, IntoExpr, Path};

/// Convert a comparison operand to the subject's application type.
///
/// Values use [`IntoExpr`]. Required paths and expressions also lift into
/// `Some` when compared with an optional subject. This conversion is separate
/// from projection so selecting a required field retains its declared type.
pub trait IntoComparison<T> {
    /// Construct the comparison operand.
    fn into_comparison(self) -> Expr<T>;
}

impl<T, E: IntoExpr<T>> IntoComparison<T> for E {
    fn into_comparison(self) -> Expr<T> {
        self.into_expr()
    }
}

impl<T> IntoComparison<Option<T>> for Expr<T> {
    fn into_comparison(self) -> Expr<Option<T>> {
        self.some()
    }
}

impl<T> IntoComparison<Option<T>> for &Expr<T> {
    fn into_comparison(self) -> Expr<Option<T>> {
        self.clone().some()
    }
}

impl<O, T> IntoComparison<Option<T>> for Path<O, T> {
    fn into_comparison(self) -> Expr<Option<T>> {
        self.into_expr().some()
    }
}
