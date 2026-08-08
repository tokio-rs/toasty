use super::OrderByExpr;

/// An `ORDER BY` clause containing one or more ordering expressions.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{OrderBy, OrderByExpr, Direction, Expr};
///
/// let order = OrderBy {
///     exprs: vec![OrderByExpr {
///         expr: Expr::null(),
///         order: Some(Direction::Asc),
///     }],
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBy {
    /// The list of ordering expressions, applied in order.
    pub exprs: Vec<OrderByExpr>,
}

impl OrderBy {
    /// Flips the direction of each [`OrderByExpr`] that makes up this [`OrderBy`].
    pub fn reverse(&mut self) {
        for expr in &mut self.exprs {
            expr.reverse();
        }
    }
}

impl From<OrderByExpr> for OrderBy {
    fn from(value: OrderByExpr) -> Self {
        Self { exprs: vec![value] }
    }
}

macro_rules! impl_for_tuple {
    ( $(($T:ident, $idx:tt)),+ ) => {
        impl<$($T),+> From<($($T,)+)> for OrderBy
        where
            $($T: Into<OrderByExpr>,)+
        {
            fn from(src: ($($T,)+)) -> Self {
                Self {
                    exprs: vec![$(src.$idx.into()),+],
                }
            }
        }
    };
}

impl_for_tuple!((T0, 0), (T1, 1));
impl_for_tuple!((T0, 0), (T1, 1), (T2, 2));
impl_for_tuple!((T0, 0), (T1, 1), (T2, 2), (T3, 3));
impl_for_tuple!((T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4));
impl_for_tuple!((T0, 0), (T1, 1), (T2, 2), (T3, 3), (T4, 4), (T5, 5));
impl_for_tuple!(
    (T0, 0),
    (T1, 1),
    (T2, 2),
    (T3, 3),
    (T4, 4),
    (T5, 5),
    (T6, 6)
);
impl_for_tuple!(
    (T0, 0),
    (T1, 1),
    (T2, 2),
    (T3, 3),
    (T4, 4),
    (T5, 5),
    (T6, 6),
    (T7, 7)
);
impl_for_tuple!(
    (T0, 0),
    (T1, 1),
    (T2, 2),
    (T3, 3),
    (T4, 4),
    (T5, 5),
    (T6, 6),
    (T7, 7),
    (T8, 8)
);
impl_for_tuple!(
    (T0, 0),
    (T1, 1),
    (T2, 2),
    (T3, 3),
    (T4, 4),
    (T5, 5),
    (T6, 6),
    (T7, 7),
    (T8, 8),
    (T9, 9)
);
impl_for_tuple!(
    (T0, 0),
    (T1, 1),
    (T2, 2),
    (T3, 3),
    (T4, 4),
    (T5, 5),
    (T6, 6),
    (T7, 7),
    (T8, 8),
    (T9, 9),
    (T10, 10)
);
impl_for_tuple!(
    (T0, 0),
    (T1, 1),
    (T2, 2),
    (T3, 3),
    (T4, 4),
    (T5, 5),
    (T6, 6),
    (T7, 7),
    (T8, 8),
    (T9, 9),
    (T10, 10),
    (T11, 11)
);
