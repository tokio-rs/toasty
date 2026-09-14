//! Attaches the variant checks a predicate over an enum variant requires.
//!
//! A path into an enum variant (`contact().email().address()`) converts to an
//! [`ExprVariant`] selection: the variant's payload, without a check that the
//! value holds that variant. A predicate over such an operand only means
//! something for rows of that variant, so normalization conjoins an
//! `is_variant` check for every selection a predicate's operands reach — on
//! either operand, and for every enclosing variant of a nested selection,
//! outermost first.
//!
//! Guards attach at each predicate boundary: the comparison, null check,
//! membership test, or other boolean operator whose operands hold the
//! selection. Boolean connectives and subqueries are boundaries of their own,
//! so `a.eq(b).not()` becomes `NOT (is_variant AND a = b)` and negates the
//! guarded comparison as a whole, while `a.is_some()` — the `IS NOT NULL`
//! predicate — becomes `is_variant AND NOT (a IS NULL)`. A selection used as
//! a value only, such as an ordering expression, stays unguarded.
//!
//! Guards a conjunction already states are not repeated below it, which keeps
//! the rewrite idempotent and lets generated `matches()` filters keep their
//! explicit `is_variant AND body` shape. Conjunctions produced by the rewrite
//! flatten into the enclosing conjunction so that a guard and the comparison
//! it scopes stay siblings, which relation lifting relies on to fold them
//! into one subquery.

use toasty_core::stmt::{
    Expr, ExprAnd, ExprNot, ExprOr, ExprVariant, Query, Statement, Visit, VisitMut,
};

use super::Normalize;

impl Normalize<'_> {
    /// Normalizes the operands of a conjunction and flattens conjunctions
    /// they produce into it.
    ///
    /// The `is_variant` operands are in force for every operand, including
    /// those the flattening adds ahead of the operands that follow.
    pub(super) fn normalize_conjunction(&mut self, and: &mut ExprAnd) {
        let depth = self.guards.len();
        self.guards
            .extend(and.operands.iter().filter(|e| is_guard(e)).cloned());

        for mut operand in std::mem::take(&mut and.operands) {
            self.visit_expr_mut(&mut operand);

            match operand {
                Expr::And(inner) => {
                    for operand in inner.operands {
                        if is_guard(&operand) && !self.guards.contains(&operand) {
                            self.guards.push(operand.clone());
                        }
                        and.operands.push(operand);
                    }
                }
                operand => and.operands.push(operand),
            }
        }

        self.guards.truncate(depth);
    }

    /// Conjoins the variant checks a predicate's operands require with the
    /// predicate, and expands a negated predicate into its `NOT` form.
    ///
    /// The operands have already been normalized: a nested predicate carries
    /// its own guards, and connectives and subqueries are not searched.
    pub(super) fn normalize_predicate_guards(&mut self, expr: &mut Expr) {
        let mut guards = vec![];
        let mut collect = CollectGuards {
            guards: &mut guards,
            in_force: &self.guards,
        };

        match expr {
            Expr::BinaryOp(e) if e.op.is_arithmetic() => return,
            Expr::AllOp(_)
            | Expr::AnyOp(_)
            | Expr::Between(_)
            | Expr::BinaryOp(_)
            | Expr::InList(_)
            | Expr::InSubquery(_)
            | Expr::Intersects(_)
            | Expr::IsNull(_)
            | Expr::IsSuperset(_)
            | Expr::IsVariant(_)
            | Expr::Like(_)
            | Expr::StartsWith(_) => collect.visit_expr(expr),
            Expr::Not(e) => collect.visit_expr(&e.expr),
            _ => return,
        }

        let mut predicate = expr.take();
        let negated = match &mut predicate {
            Expr::IsNull(e) => std::mem::take(&mut e.negated),
            Expr::InSubquery(e) => std::mem::take(&mut e.negated),
            _ => false,
        };
        if negated {
            predicate = Expr::not(predicate);
        }

        *expr = if guards.is_empty() {
            predicate
        } else {
            guards.push(predicate);
            Expr::and_from_vec(guards)
        };
    }
}

fn is_guard(expr: &Expr) -> bool {
    matches!(expr, Expr::IsVariant(_))
}

/// Collects the `is_variant` checks for the selections an operand reaches.
///
/// Outer selections come before the selections nested inside them, without
/// duplicates and without the checks already in force. Connectives and
/// subqueries are predicate scopes of their own and are not entered.
struct CollectGuards<'a> {
    guards: &'a mut Vec<Expr>,
    in_force: &'a [Expr],
}

impl Visit for CollectGuards<'_> {
    fn visit_expr_variant(&mut self, i: &ExprVariant) {
        // The base holds the enclosing selections, so its guards come first.
        self.visit_expr(&i.base);

        let guard = Expr::is_variant((*i.base).clone(), i.variant);
        if !self.in_force.contains(&guard) && !self.guards.contains(&guard) {
            self.guards.push(guard);
        }
    }

    fn visit_expr_and(&mut self, _: &ExprAnd) {}

    fn visit_expr_or(&mut self, _: &ExprOr) {}

    fn visit_expr_not(&mut self, _: &ExprNot) {}

    fn visit_stmt(&mut self, _: &Statement) {}

    fn visit_stmt_query(&mut self, _: &Query) {}
}
