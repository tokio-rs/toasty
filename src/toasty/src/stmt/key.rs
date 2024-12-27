use super::*;

pub struct Key<T: ?Sized> {
    pub(crate) untyped: stmt::Value,

    pub(crate) _p: PhantomData<T>,
}

impl<M: Model> Key<M> {
    pub fn from_expr(expr: impl IntoExpr<M::Key>) -> Key<M> {
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

                stmt::ValueRecord::from_vec(f).into()
            }
            _ => todo!(),
        };

        Key {
            untyped,
            _p: PhantomData,
        }
    }
}

impl<M> From<Key<M>> for Expr<M> {
    fn from(value: Key<M>) -> Self {
        Expr {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<M> From<Key<M>> for Expr<[M]> {
    fn from(value: Key<M>) -> Self {
        Expr {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}
