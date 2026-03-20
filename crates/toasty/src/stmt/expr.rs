use super::{Insert, IntoExpr, List};
use std::marker::PhantomData;
use std::ops::Not;
use toasty_core::stmt;

/// A typed expression in the Toasty query language.
///
/// `Expr<T>` wraps an untyped AST expression node and tags it with a Rust type
/// `T` that represents the expression's value type. Common instantiations:
///
/// - `Expr<bool>` — a boolean filter expression (comparisons, `and`, `or`, `not`).
/// - `Expr<String>`, `Expr<i64>`, etc. — scalar value expressions.
/// - `Expr<Option<T>>` — a nullable expression with [`is_none`](Expr::is_none)
///   and [`is_some`](Expr::is_some) helpers.
/// - `Expr<List<T>>` — a list expression (see [`Expr::list`]).
///
/// Expressions are built from [`Path`] comparisons, literal values via
/// [`IntoExpr`], and combinators like [`and`](Expr::and) and [`or`](Expr::or).
#[derive(Debug)]
pub struct Expr<T> {
    pub(crate) untyped: stmt::Expr,
    pub(crate) _p: PhantomData<T>,
}

impl<T> Expr<T> {
    /// Create an expression from the given value.
    pub(crate) fn from_value(value: stmt::Value) -> Self {
        Self {
            untyped: stmt::Expr::Value(value),
            _p: PhantomData,
        }
    }

    /// Wrap a raw untyped expression, tagging it with type `T`.
    pub fn from_untyped(untyped: impl Into<stmt::Expr>) -> Self {
        Self {
            untyped: untyped.into(),
            _p: PhantomData,
        }
    }

    /// Re-tag this expression with a different type `U`.
    ///
    /// This performs no runtime conversion — the underlying AST node is
    /// unchanged. Use this when the type system needs a different phantom tag
    /// but the expression itself is compatible (e.g., widening `Expr<T>` to
    /// `Expr<Option<T>>`).
    pub fn cast<U>(self) -> Expr<U> {
        Expr {
            untyped: self.untyped,
            _p: PhantomData,
        }
    }
}

impl<T> Expr<List<T>> {
    /// Build a list expression from an iterator of items.
    ///
    /// Each item is converted to an `Expr<T>` via [`IntoExpr`]. The resulting
    /// expression represents a literal list value.
    ///
    /// ```ignore
    /// let ids = Expr::<List<i64>>::list([1, 2, 3]);
    /// ```
    pub fn list<I>(items: impl IntoIterator<Item = I>) -> Self
    where
        I: IntoExpr<T>,
    {
        Self::from_untyped(stmt::Expr::list(
            items.into_iter().map(|item| item.into_expr().untyped),
        ))
    }
}

impl Expr<bool> {
    /// Combine two boolean expressions with logical AND.
    ///
    /// ```ignore
    /// let filter = User::fields().name().eq("Alice")
    ///     .and(User::fields().age().gt(18));
    /// ```
    pub fn and(self, rhs: impl IntoExpr<bool>) -> Self {
        Self::from_untyped(stmt::Expr::and(self.untyped, rhs.into_expr().untyped))
    }

    /// Combine an iterator of boolean expressions with logical AND.
    ///
    /// Returns `true` (no filter) when the iterator is empty.
    pub fn and_all<E>(exprs: impl IntoIterator<Item = E>) -> Self
    where
        E: IntoExpr<bool>,
    {
        exprs
            .into_iter()
            .map(|expr| expr.into_expr().untyped)
            .reduce(stmt::Expr::and)
            .map(Self::from_untyped)
            .unwrap_or_else(|| Self::from_untyped(true))
    }

    /// Combine two boolean expressions with logical OR.
    pub fn or(self, rhs: impl IntoExpr<bool>) -> Self {
        Self::from_untyped(stmt::Expr::or(self.untyped, rhs.into_expr().untyped))
    }

    /// Negate this boolean expression.
    ///
    /// Equivalent to the `!` operator (which is also implemented via [`Not`]).
    #[allow(clippy::should_implement_trait)]
    pub fn not(self) -> Self {
        !self
    }

    /// Test whether `lhs` is contained in `rhs`.
    ///
    /// This is the associated-function form of [`in_list`](super::in_list).
    /// Both single values and tuples (composite keys) are supported.
    pub fn in_list<L, R, T>(lhs: L, rhs: R) -> Self
    where
        L: IntoExpr<T>,
        R: IntoExpr<List<T>>,
    {
        Self::from_untyped(stmt::Expr::in_list(
            lhs.into_expr().untyped,
            rhs.into_expr().untyped,
        ))
    }
}

impl Not for Expr<bool> {
    type Output = Self;

    fn not(self) -> Self {
        Self::from_untyped(stmt::Expr::not(self.untyped))
    }
}

impl<T> Expr<Option<T>> {
    /// Test whether this optional expression is `NULL`.
    pub fn is_none(self) -> Expr<bool> {
        Expr::from_untyped(stmt::Expr::is_null(self.untyped))
    }

    /// Test whether this optional expression is not `NULL`.
    pub fn is_some(self) -> Expr<bool> {
        Expr::from_untyped(stmt::Expr::is_not_null(self.untyped))
    }
}

impl<T> Clone for Expr<T> {
    fn clone(&self) -> Self {
        Self {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<T> From<Expr<T>> for stmt::Expr {
    fn from(value: Expr<T>) -> Self {
        value.untyped
    }
}

impl<T> From<Insert<T>> for Expr<T> {
    fn from(value: Insert<T>) -> Self {
        Self::from_untyped(stmt::Expr::Stmt(value.untyped.into()))
    }
}

impl<T> From<Insert<T>> for Expr<Option<T>> {
    fn from(value: Insert<T>) -> Self {
        Self::from_untyped(stmt::Expr::Stmt(value.untyped.into()))
    }
}
