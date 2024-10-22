use super::*;

pub trait IntoExpr<'a, T: ?Sized> {
    fn into_expr(self) -> Expr<'a, T>;
}

macro_rules! impl_into_expr_for_copy {
    ( $( $var:ident($t:ty) ;)* ) => {
        $(
            impl<'a> IntoExpr<'a, $t> for $t {
                fn into_expr(self) -> Expr<'a, $t> {
                    Expr::from_value(Value::from(self))
                }
            }

            impl<'a> IntoExpr<'a, $t> for &'a $t {
                fn into_expr(self) -> Expr<'a, $t> {
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

impl<'a, T: ?Sized> IntoExpr<'a, T> for Expr<'a, T> {
    fn into_expr(self) -> Expr<'a, T> {
        self
    }
}

impl<'a> IntoExpr<'a, String> for &'a str {
    fn into_expr(self) -> Expr<'a, String> {
        Expr::from_value(Value::from(self))
    }
}

impl<'a> IntoExpr<'a, String> for &'a String {
    fn into_expr(self) -> Expr<'a, String> {
        Expr::from_value(Value::from(self))
    }
}

impl IntoExpr<'static, String> for String {
    fn into_expr(self) -> Expr<'static, String> {
        Expr::from_value(self.into())
    }
}

impl<'a, T: Model> IntoExpr<'a, Id<T>> for Id<T> {
    fn into_expr(self) -> Expr<'a, Id<T>> {
        Expr::from_value(self.inner.into())
    }
}

impl<'a, T: Model> IntoExpr<'a, Id<T>> for &'a Id<T> {
    fn into_expr(self) -> Expr<'a, Id<T>> {
        Expr::from_value(Value::from(&self.inner))
    }
}

impl<'a, T1, T2> IntoExpr<'a, (T1,)> for (T2,)
where
    T2: IntoExpr<'a, T1>,
{
    fn into_expr(self) -> Expr<'a, (T1,)> {
        let record = stmt::ExprRecord::from_vec(vec![self.0.into_expr().untyped]);
        let untyped = stmt::Expr::Record(record);
        Expr::from_untyped(untyped)
    }
}

impl<'a, T, U> IntoExpr<'a, [T]> for Vec<U>
where
    U: IntoExpr<'a, T>,
{
    fn into_expr(self) -> Expr<'a, [T]> {
        Expr::from_untyped(stmt::Expr::list(
            self.into_iter().map(|item| item.into_expr().untyped),
        ))
    }
}

impl<'a, T1, T2, U1, U2> IntoExpr<'a, (T1, U1)> for (T2, U2)
where
    T2: IntoExpr<'a, T1>,
    U2: IntoExpr<'a, U1>,
{
    fn into_expr(self) -> Expr<'a, (T1, U1)> {
        let record = stmt::ExprRecord::from_vec(vec![
            self.0.into_expr().untyped,
            self.1.into_expr().untyped,
        ]);
        let untyped = stmt::Expr::Record(record);
        Expr::from_untyped(untyped)
    }
}
