use super::Expr;

/// A map/transform operation over a collection.
///
/// [`ExprMap`] applies a transformation expression to each item in a base
/// collection. Within the `map` expression, `Expr::arg(n)` refers to elements
/// of each item:
///
/// - For simple values, `arg(0)` is the item itself.
/// - For records, `arg(0)` is field 0, `arg(1)` is field 1, etc.
///
/// # Examples
///
/// ## Simple values
///
/// ```text
/// map([1, 2, 3], x => x == field)
/// ```
///
/// Here `base` is `[1, 2, 3]` and `map` is `arg(0) == field`.
///
/// ## Records
///
/// ```text
/// map([{1, 2}, {3, 4}], r => r.0 + r.1)
/// ```
///
/// Here each item is a record with two fields. `arg(0)` refers to the first
/// field and `arg(1)` refers to the second field of each record.
#[derive(Debug, Clone, PartialEq)]
pub struct ExprMap {
    /// The collection expression to iterate over.
    pub base: Box<Expr>,

    /// The transformation to apply to each item. Use `Expr::arg(n)` to
    /// reference elements of the current item being mapped.
    pub map: Box<Expr>,
}

impl Expr {
    pub fn map(base: impl Into<Self>, map: impl Into<Self>) -> Self {
        ExprMap {
            base: Box::new(base.into()),
            map: Box::new(map.into()),
        }
        .into()
    }

    pub fn as_map(&self) -> &ExprMap {
        match self {
            Self::Map(expr) => expr,
            _ => todo!(),
        }
    }
}

impl From<ExprMap> for Expr {
    fn from(value: ExprMap) -> Self {
        Self::Map(value)
    }
}
