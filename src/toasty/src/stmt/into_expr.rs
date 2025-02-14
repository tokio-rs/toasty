use super::*;

pub trait IntoExpr<T: ?Sized> {
    fn into_expr(self) -> Expr<T>;
}

macro_rules! impl_into_expr_for_copy {
    ( $( $var:ident($t:ty) ;)* ) => {
        $(
            impl IntoExpr<$t> for $t {
                fn into_expr(self) -> Expr<$t> {
                    Expr::from_value(Value::from(self))
                }
            }

            impl IntoExpr<$t> for &$t {
                fn into_expr(self) -> Expr<$t> {
                    Expr::from_value(Value::from(*self))
                }
            }
        )*
    };
}

impl_into_expr_for_copy! {
    Bool(bool);
    I64(i64);
}

impl<T: ?Sized> IntoExpr<T> for Expr<T> {
    fn into_expr(self) -> Expr<T> {
        self
    }
}

impl IntoExpr<String> for &str {
    fn into_expr(self) -> Expr<String> {
        Expr::from_value(Value::from(self))
    }
}

impl IntoExpr<String> for &String {
    fn into_expr(self) -> Expr<String> {
        Expr::from_value(Value::from(self))
    }
}

impl IntoExpr<String> for String {
    fn into_expr(self) -> Expr<String> {
        Expr::from_value(self.into())
    }
}

impl<T1, T2> IntoExpr<T1> for Option<T2>
where
    T2: IntoExpr<T1>,
{
    fn into_expr(self) -> Expr<T1> {
        match self {
            Some(value) => value.into_expr(),
            None => Expr::from_value(Value::Null),
        }
    }
}

impl<T1, T2> IntoExpr<T1> for &Option<T2>
where
    for<'a> &'a T2: IntoExpr<T1>,
{
    fn into_expr(self) -> Expr<T1> {
        match self {
            Some(value) => value.into_expr(),
            None => Expr::from_value(Value::Null),
        }
    }
}

impl<T, U, const N: usize> IntoExpr<[T]> for &[U; N]
where
    for<'a> &'a U: IntoExpr<T>,
{
    fn into_expr(self) -> Expr<[T]> {
        Expr::list(self)
    }
}

impl<T1, T2> IntoExpr<(T1,)> for (T2,)
where
    T2: IntoExpr<T1>,
{
    fn into_expr(self) -> Expr<(T1,)> {
        let record = stmt::ExprRecord::from_vec(vec![self.0.into_expr().untyped]);
        let untyped = stmt::Expr::Record(record);
        Expr::from_untyped(untyped)
    }
}

impl<T, U> IntoExpr<[T]> for Vec<U>
where
    U: IntoExpr<T>,
{
    fn into_expr(self) -> Expr<[T]> {
        Expr::from_untyped(stmt::Expr::list(
            self.into_iter().map(|item| item.into_expr().untyped),
        ))
    }
}

impl<T1, T2, U1, U2> IntoExpr<(T1, U1)> for (T2, U2)
where
    T2: IntoExpr<T1>,
    U2: IntoExpr<U1>,
{
    fn into_expr(self) -> Expr<(T1, U1)> {
        let record = stmt::ExprRecord::from_vec(vec![
            self.0.into_expr().untyped,
            self.1.into_expr().untyped,
        ]);
        let untyped = stmt::Expr::Record(record);
        Expr::from_untyped(untyped)
    }
}
