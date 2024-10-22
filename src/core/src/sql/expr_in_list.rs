use super::*;

#[derive(Debug, Clone)]
pub struct ExprInList<'stmt> {
    pub expr: Box<Expr<'stmt>>,
    pub list: ExprList<'stmt>,
}

impl<'stmt> ExprInList<'stmt> {
    pub(crate) fn substitute_ref(
        &mut self,
        input: &mut impl substitute::Input<'stmt>,
    ) -> Option<Expr<'stmt>> {
        self.expr.substitute_ref(input);

        match &mut self.list {
            ExprList::Expr(exprs) => {
                for expr in exprs {
                    expr.substitute_ref(input);
                }
            }
            ExprList::Value(_) => {}
            ExprList::Placeholder(expr_placeholder) => {
                match input.resolve_placeholder(expr_placeholder) {
                    Expr::Value(stmt::Value::List(values)) => {
                        assert_ne!(values.len(), 0);

                        if values.len() == 1 {
                            let lhs = std::mem::take(&mut *self.expr);
                            let rhs = values.into_iter().next().unwrap();

                            return Some(Expr::eq(lhs, rhs));
                        } else {
                            self.list = ExprList::Value(values)
                        }
                    }
                    expr => todo!("expr={:#?}", expr),
                };
            }
        }

        None
    }
}
