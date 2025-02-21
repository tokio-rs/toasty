use super::*;

pub trait IntoExpr<T: ?Sized> {
    fn into_expr(self) -> Expr<T>;

    fn by_ref(&self) -> Expr<T>;
}

macro_rules! impl_into_expr_for_copy {
    ( $( $var:ident($t:ty) ;)* ) => {
        $(
            impl IntoExpr<$t> for $t {
                fn into_expr(self) -> Expr<$t> {
                    Expr::from_value(Value::from(self))
                }

                fn by_ref(&self) -> Expr<$t> {
                    Expr::from_value(Value::from(self.clone()))
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

    fn by_ref(&self) -> Expr<T> {
        self.clone()
    }
}

impl<T, E> IntoExpr<T> for &E
where
    T: ?Sized,
    E: IntoExpr<T>,
{
    fn into_expr(self) -> Expr<T> {
        IntoExpr::by_ref(self)
    }

    fn by_ref(&self) -> Expr<T> {
        IntoExpr::by_ref(*self)
    }
}

impl IntoExpr<String> for &str {
    fn into_expr(self) -> Expr<String> {
        Expr::from_value(Value::from(self))
    }

    fn by_ref(&self) -> Expr<String> {
        Expr::from_value(Value::from(*self))
    }
}

impl IntoExpr<String> for String {
    fn into_expr(self) -> Expr<String> {
        Expr::from_value(self.into())
    }

    fn by_ref(&self) -> Expr<String> {
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

    fn by_ref(&self) -> Expr<T1> {
        match self {
            Some(value) => IntoExpr::by_ref(value),
            None => Expr::from_value(Value::Null),
        }
    }
}

impl<T, U, const N: usize> IntoExpr<[T]> for [U; N]
where
    U: IntoExpr<T>,
{
    fn into_expr(self) -> Expr<[T]> {
        Expr::list(&self)
    }

    fn by_ref(&self) -> Expr<[T]> {
        Expr::list(self)
    }
}

impl<T, E: IntoExpr<T>> IntoExpr<[T]> for &[E] {
    fn into_expr(self) -> Expr<[T]> {
        Expr::list(self)
    }

    fn by_ref(&self) -> Expr<[T]> {
        Expr::list(*self)
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

    fn by_ref(&self) -> Expr<(T1,)> {
        todo!()
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

    fn by_ref(&self) -> Expr<[T]> {
        todo!()
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

    fn by_ref(&self) -> Expr<(T1, U1)> {
        (&self.0, &self.1).into_expr()
    }
}
