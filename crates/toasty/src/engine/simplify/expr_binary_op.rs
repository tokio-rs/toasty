use super::Simplify;
use toasty_core::stmt::{self, Expr, ResolvedRef, VisitMut};

impl Simplify<'_> {
    /// Heavyweight binary-op rewrites. Cheap canonicalization (constant
    /// folding, null propagation, boolean-constant simplification,
    /// literal-on-right swap) runs in `fold::expr_binary_op` before this
    /// is reached, so heavyweight rules see operands in canonical form
    /// (no `(Value, Value)`, no `(Value, _)` ahead of `(_, Value)`).
    ///
    /// App-level rewrites on eq/ne operands (`Reference::Model` →
    /// primary-key field, `BelongsTo` → foreign-key field) fire in the
    /// pre-lowering `lower::expr_eq_operand::RewriteEqOperand` pass, not
    /// here.
    pub(super) fn simplify_expr_binary_op(
        &mut self,
        op: stmt::BinaryOp,
        lhs: &mut stmt::Expr,
        rhs: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
        let result = match (&mut *lhs, &mut *rhs) {
            // Self-comparison, e.g.,
            //
            //  - `x = x` → `true`
            //  - `x != x` → `false`
            //
            // Only applied for non-nullable field references.
            (Expr::Reference(lhs), Expr::Reference(rhs))
                if lhs == rhs && (op.is_eq() || op.is_ne()) =>
            {
                if lhs.is_field() {
                    let field = self.cx.resolve_expr_reference(lhs).as_field_unwrap();
                    if !field.nullable() {
                        return Some(op.is_eq().into());
                    }
                }
                None
            }
            // Tuple decomposition,
            //
            //  - `(a, b) = (x, y)` → `a = x and b = y`
            //  - `(a, b) != (x, y)` → `a != x or b != y`
            (Expr::Record(lhs_rec), Expr::Record(rhs_rec))
                if (op.is_eq() || op.is_ne()) && lhs_rec.len() == rhs_rec.len() =>
            {
                let comparisons: Vec<_> = std::mem::take(&mut lhs_rec.fields)
                    .into_iter()
                    .zip(std::mem::take(&mut rhs_rec.fields))
                    .map(|(l, r)| Expr::binary_op(l, op, r))
                    .collect();

                if op.is_eq() {
                    Some(Expr::and_from_vec(comparisons))
                } else {
                    Some(Expr::or_from_vec(comparisons))
                }
            }
            // Tuple decomposition with a Value::Record on one side,
            //
            //  - `(a, b) = Value::Record([x, y])` → `a = x and b = y`
            //
            // This arises after match elimination produces `Record([col1, col2]) == Value::Record([1, "alice"])`.
            (Expr::Record(rec), Expr::Value(stmt::Value::Record(val_rec)))
            | (Expr::Value(stmt::Value::Record(val_rec)), Expr::Record(rec))
                if (op.is_eq() || op.is_ne()) && rec.len() == val_rec.len() =>
            {
                let comparisons: Vec<_> = std::mem::take(&mut rec.fields)
                    .into_iter()
                    .zip(std::mem::take(&mut val_rec.fields))
                    .map(|(expr, val)| Expr::binary_op(expr, op, Expr::from(val)))
                    .collect();

                if op.is_eq() {
                    Some(Expr::and_from_vec(comparisons))
                } else {
                    Some(Expr::or_from_vec(comparisons))
                }
            }
            // Match elimination: distribute binary op into match arms as OR
            //
            //   Match(subj, [p1 => e1, p2 => e2]) <op> rhs
            //   → OR(subj == p1 AND e1 <op> rhs, subj == p2 AND e2 <op> rhs)
            //
            // Each arm is fully simplified inline. Arms that fold to false/null
            // are pruned. Comparison ops only — distributing arithmetic this
            // way would produce a malformed boolean from a non-boolean term.
            (Expr::Match(m), _) if !op.is_arithmetic() && m.subject.is_stable() => {
                let match_expr = lhs.take();
                let other = rhs.take();
                Some(self.eliminate_match_in_binary_op(op, match_expr, other, true))
            }
            (_, Expr::Match(m)) if !op.is_arithmetic() && m.subject.is_stable() => {
                let other = lhs.take();
                let match_expr = rhs.take();
                Some(self.eliminate_match_in_binary_op(op, match_expr, other, false))
            }
            // Self-comparison with projections, e.g.,
            //
            //  - `address.city = address.city` → `true`
            //  - `address.city != address.city` → `false`
            //
            // By this point, constant projections and record projections have been simplified.
            // What remains are projections with opaque bases (e.g., field references).
            // `lhs.base.is_stable()` keeps this sound: a projection through a
            // non-deterministic base would evaluate the base twice and could
            // yield different values each time.
            (Expr::Project(lhs), Expr::Project(rhs))
                if lhs == rhs && lhs.base.is_stable() && (op.is_eq() || op.is_ne()) =>
            {
                // TODO: Check if the projected value is nullable
                Some(Expr::from(op.is_eq()))
            }
            _ => None,
        };

        if result.is_some() {
            return result;
        }

        // Null propagation for derived VALUES columns.
        //
        // If either operand is a column reference into a derived VALUES
        // table where every row has NULL at that column position, the
        // binary op can never produce a non-null result.
        if self.is_always_null_derived_column(lhs) || self.is_always_null_derived_column(rhs) {
            return Some(Expr::null());
        }

        // Relation-path-comparison and IN-subquery lifting fire in the
        // pre-lowering `lower::lift_in_subquery::*` pass, not here.
        None
    }

    /// Returns `true` if `expr` is a column reference that resolves to a
    /// derived VALUES table where every row has NULL at the referenced column.
    fn is_always_null_derived_column(&self, expr: &Expr) -> bool {
        let Expr::Reference(expr_ref) = expr else {
            return false;
        };

        match self.cx.resolve_expr_reference(expr_ref) {
            ResolvedRef::Derived(derived_ref) => derived_ref.is_column_always_null(),
            _ => false,
        }
    }

    /// Distributes a binary op over match arms, producing an OR of guarded
    /// comparisons. Each arm becomes `(subject == pattern) AND (arm_expr <op> other)`.
    /// Dead branches (false/null) are pruned after inline simplification.
    fn eliminate_match_in_binary_op(
        &mut self,
        op: stmt::BinaryOp,
        match_expr: Expr,
        other: Expr,
        match_on_lhs: bool,
    ) -> Expr {
        let Expr::Match(match_expr) = match_expr else {
            unreachable!()
        };

        let mut operands = Vec::new();

        // Collect arm patterns before consuming the arms (needed for the else guard).
        let patterns: Vec<_> = match_expr.arms.iter().map(|a| a.pattern.clone()).collect();

        for arm in match_expr.arms {
            let guard = Expr::binary_op(
                (*match_expr.subject).clone(),
                stmt::BinaryOp::Eq,
                Expr::from(arm.pattern),
            );

            let comparison = if match_on_lhs {
                Expr::binary_op(arm.expr, op, other.clone())
            } else {
                Expr::binary_op(other.clone(), op, arm.expr)
            };

            let mut term = Expr::and_from_vec(vec![guard, comparison]);
            self.visit_expr_mut(&mut term);

            // Prune dead branches
            if is_dead_filter_term(&term) {
                continue;
            }

            operands.push(term);
        }

        // Include the else branch with a guard that negates all arm patterns.
        {
            let guards: Vec<Expr> = patterns
                .into_iter()
                .map(|pattern| {
                    Expr::not(Expr::binary_op(
                        (*match_expr.subject).clone(),
                        stmt::BinaryOp::Eq,
                        Expr::from(pattern),
                    ))
                })
                .collect();

            let comparison = if match_on_lhs {
                Expr::binary_op(*match_expr.else_expr, op, other)
            } else {
                Expr::binary_op(other, op, *match_expr.else_expr)
            };

            let mut else_operands = guards;
            else_operands.push(comparison);
            let mut term = Expr::and_from_vec(else_operands);
            self.visit_expr_mut(&mut term);

            // Prune dead branches
            if !is_dead_filter_term(&term) {
                operands.push(term);
            }
        }

        Expr::or_from_vec(operands)
    }
}

/// Returns `true` when a distributed-comparison term can never be `true`, so it
/// adds nothing to the surrounding `OR` and can be dropped.
///
/// Three shapes qualify:
///
/// 1. The term is `false` or a bare `NULL`.
///
/// 2. The term is an `AND` with a `NULL` conjunct, such as `x AND NULL`. This
///    comes from comparing against a decode `Match`'s `null` else branch — for
///    example `eq(option_embed, Some(..))` on a `None` row. `x AND NULL` never
///    holds in three-valued logic, and DynamoDB rejects the bare `NULL`
///    placeholder it would otherwise serialize to.
///
/// 3. The term compares against an `Expr::Error`, the placeholder a total enum
///    decode puts in its unreachable else branch. For a two-variant enum,
///    distributing `field == "x"` over the decode produces a term like
///    `disc != 1 AND disc != 2 AND Error == "x"`. A surrounding variant gate
///    usually contradicts the `disc != k` guards and folds this away, but
///    factoring a shared predicate out from under its gates removes that gate
///    (issue #1061). Since `Error` marks a branch that never runs, the
///    comparison can never hold. Dropping the term also keeps `Error` out of
///    the SQL serializer, which cannot render it.
fn is_dead_filter_term(term: &Expr) -> bool {
    if term.is_unsatisfiable() {
        return true;
    }
    if contains_error(term) {
        return true;
    }
    matches!(
        term,
        Expr::And(and) if and.operands.iter().any(Expr::is_value_null)
    )
}

/// Returns `true` if `expr` contains an `Expr::Error` node anywhere in its
/// subtree — the unreachable-branch placeholder produced by enum decode.
fn contains_error(expr: &Expr) -> bool {
    use toasty_core::stmt::Visit;

    struct FindError(bool);

    impl Visit for FindError {
        fn visit_expr_error(&mut self, _: &stmt::ExprError) {
            self.0 = true;
        }
    }

    let mut find = FindError(false);
    find.visit_expr(expr);
    find.0
}
