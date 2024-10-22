use super::*;

pub struct Key<'stmt, T: ?Sized> {
    pub(crate) untyped: stmt::Value<'stmt>,

    pub(crate) _p: PhantomData<T>,
}

impl<'stmt, M: Model> Key<'stmt, M> {
    pub fn from_expr(expr: impl IntoExpr<'stmt, M::Key>) -> Key<'stmt, M> {
        let expr = expr.into_expr();

        let untyped = match expr.untyped {
            stmt::Expr::Value(value) => value,
            stmt::Expr::Record(fields) => {
                assert!(fields.len() > 1);

                let mut f = vec![];

                for field in fields {
                    match field {
                        stmt::Expr::Value(value) => f.push(value),
                        _ => todo!(),
                    }
                }

                stmt::Record::from_vec(f).into()
            }
            _ => todo!(),
        };

        Key {
            untyped,
            _p: PhantomData,
        }
    }
}

impl<'stmt, M> From<Key<'stmt, M>> for Expr<'stmt, M> {
    fn from(value: Key<'stmt, M>) -> Self {
        Expr {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<'stmt, M> From<Key<'stmt, M>> for Expr<'stmt, [M]> {
    fn from(value: Key<'stmt, M>) -> Self {
        Expr {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}
