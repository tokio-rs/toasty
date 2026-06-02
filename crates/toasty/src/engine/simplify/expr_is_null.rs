use super::Simplify;
use toasty_core::stmt::{self, Expr, VisitMut};

impl Simplify<'_> {
    /// Heavyweight `IS NULL` rewrites. Cheap canonicalization (constant
    /// folding, cast stripping) runs in `fold::expr_is_null` before this is
    /// reached.
    pub(super) fn simplify_expr_is_null(&mut self, expr: &mut stmt::ExprIsNull) -> Option<Expr> {
        match &mut *expr.expr {
            Expr::Reference(f @ stmt::ExprReference::Field { .. }) => {
                let field = self.cx.resolve_expr_reference(f).as_field_unwrap();

                if !field.nullable() {
                    // `is_null` on a non-nullable field evaluates to `false`.
                    return Some(Expr::Value(stmt::Value::Bool(false)));
                }

                None
            }
            // A flattened embed reference lowers to a record of its columns; the
            // embed is absent exactly when every column is NULL. Distribute so
            // the predicate reduces to a conjunction of per-column `IS NULL`
            // (the SQL serializer has no notion of "record IS NULL").
            Expr::Record(rec) => {
                let tests: Vec<Expr> = std::mem::take(&mut rec.fields)
                    .into_iter()
                    .map(Expr::is_null)
                    .collect();
                let mut result = Expr::and_from_vec(tests);
                self.visit_expr_mut(&mut result);
                Some(result)
            }
            // A nullable embed's field reference lowers to a `Match` sentinel
            // (`Match(all_null, [true => null], record)`). SQL predicates can't
            // carry a `Match`, so distribute `IS NULL` over its arms into an OR
            // of guarded `IS NULL` terms, mirroring the binary-op match
            // elimination. Requires a stable subject so the guards may be
            // duplicated safely.
            Expr::Match(m) if m.subject.is_stable() => {
                let Expr::Match(m) = expr.expr.take() else {
                    unreachable!()
                };
                Some(self.eliminate_match_in_is_null(m))
            }
            _ => None,
        }
    }

    /// Distributes `IS NULL` over match arms, producing an OR of guarded
    /// `IS NULL` terms. Each arm becomes `(subject == pattern) AND is_null(arm_expr)`,
    /// plus an else term guarded by the negation of every arm pattern. Dead
    /// branches (false/null) are pruned after inline simplification.
    fn eliminate_match_in_is_null(&mut self, match_expr: stmt::ExprMatch) -> Expr {
        let mut operands = Vec::new();

        let patterns: Vec<_> = match_expr.arms.iter().map(|a| a.pattern.clone()).collect();

        for arm in match_expr.arms {
            let guard = Expr::eq((*match_expr.subject).clone(), Expr::from(arm.pattern));
            let mut term = Expr::and_from_vec(vec![guard, Expr::is_null(arm.expr)]);
            self.visit_expr_mut(&mut term);

            if term.is_false() || matches!(&term, Expr::Value(stmt::Value::Null)) {
                continue;
            }

            operands.push(term);
        }

        {
            let mut else_operands: Vec<Expr> = patterns
                .into_iter()
                .map(|pattern| {
                    Expr::not(Expr::eq((*match_expr.subject).clone(), Expr::from(pattern)))
                })
                .collect();
            else_operands.push(Expr::is_null(*match_expr.else_expr));

            let mut term = Expr::and_from_vec(else_operands);
            self.visit_expr_mut(&mut term);

            if !term.is_false() && !matches!(&term, Expr::Value(stmt::Value::Null)) {
                operands.push(term);
            }
        }

        Expr::or_from_vec(operands)
    }
}
