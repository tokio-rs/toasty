use super::Simplify;
use toasty_core::stmt::{self, Expr, ResolvedRef};

impl Simplify<'_> {
    /// Post-lower rewrites of `<expr> IN (list)` whose subject is a column
    /// decode: the shapes an embedded relation resolves to once its key
    /// projection lowers to storage.
    pub(super) fn simplify_expr_in_list(&mut self, expr: &mut stmt::ExprInList) -> Option<Expr> {
        match &mut *expr.expr {
            // An enum decode `Match` as the subject: distribute membership
            // over the arms, as `simplify_expr_binary_op` does for
            // comparisons.
            Expr::Match(m) if m.subject.is_stable() => {
                let match_expr = expr.expr.take();
                let list = expr.list.take();
                Some(self.eliminate_match(match_expr, |arm| Expr::in_list(arm, list.clone())))
            }
            // Decode-cast stripping: `cast(col, T) IN (values)` compares the
            // stored form directly, with the values converted to the column
            // type — the list-membership form of the cast-versus-value rule.
            Expr::Cast(cast) if cast.from.is_none() && cast.expr.is_column() => {
                self.strip_decode_cast_in_list(cast, &mut expr.list)
            }
            _ => None,
        }
    }

    fn strip_decode_cast_in_list(
        &mut self,
        cast: &mut stmt::ExprCast,
        list: &mut Expr,
    ) -> Option<Expr> {
        let expr_reference = cast.expr.as_expr_reference()?;

        let ResolvedRef::Column(column) = self.cx.resolve_expr_reference(expr_reference) else {
            return None;
        };

        let schema = self.cx.schema();
        match list {
            Expr::Value(stmt::Value::List(values)) => {
                for value in values {
                    *value = column
                        .ty
                        .cast(schema, value.take())
                        .expect("failed to cast value");
                }
            }
            Expr::List(expr_list) => {
                for item in &mut expr_list.items {
                    let Expr::Value(value) = item else {
                        return None;
                    };
                    *value = column
                        .ty
                        .cast(schema, value.take())
                        .expect("failed to cast value");
                }
            }
            _ => return None,
        }

        Some(Expr::in_list(cast.expr.take(), list.take()))
    }
}
