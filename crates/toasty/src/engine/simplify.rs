mod expr_and;
mod expr_any;
mod expr_binary_op;
mod expr_cast;
mod expr_exists;
mod expr_intersects;
mod expr_is_null;
mod expr_is_superset;
mod expr_let;
mod expr_list;
mod expr_map;
mod expr_or;
mod expr_project;
mod stmt_query;

// Simplifications
// TODO: unify names
mod lift_in_subquery;
use toasty_core::{
    driver::Capability,
    schema::*,
    stmt::{self, Expr, IntoExprTarget, Node, VisitMut},
};

use crate::engine::{Engine, fold};

/// Statement and expression simplifier.
///
/// [`Simplify`] implements the [`VisitMut`] trait to traverse and transform
/// statement ASTs. It applies optimization and normalization rules defined in
/// submodules of [`engine::simplify`](self).
///
/// Simplification runs twice during query compilation: once before lowering
/// (to normalize the input) and once after (to clean up generated expressions).
pub(crate) struct Simplify<'a> {
    /// Expression context providing schema access and type information.
    cx: stmt::ExprContext<'a>,
    /// Driver capabilities, consulted by passes that emit driver-specific shapes.
    capability: &'a Capability,
}

impl Engine {
    /// Simplifies a statement or expression in place.
    pub(crate) fn simplify_stmt<T: Node>(&self, stmt: &mut T) {
        Simplify::new(&self.schema, self.capability).visit_mut(stmt);
    }
}

/// Simplifies an expression in place using the given context and capability.
pub(crate) fn simplify_expr(
    cx: stmt::ExprContext<'_>,
    capability: &Capability,
    expr: &mut stmt::Expr,
) {
    Simplify { cx, capability }.visit_expr_mut(expr);
}

impl VisitMut for Simplify<'_> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        // Recurse into children first.
        stmt::visit_mut::visit_expr_mut(self, i);

        // Fold this node bottom-up so heavyweight rules see canonical input.
        // Children are already canonical (post-order recursion), so this is
        // effectively local.
        fold::fold_stmt(i);

        let maybe_expr = match i {
            Expr::Any(expr) => self.simplify_expr_any(expr),
            Expr::And(expr) => self.simplify_expr_and(expr),
            Expr::BinaryOp(expr) => {
                self.simplify_expr_binary_op(expr.op, &mut expr.lhs, &mut expr.rhs)
            }
            Expr::Cast(expr) => self.simplify_expr_cast(expr),
            Expr::Exists(expr) => self.simplify_expr_exists(expr),
            Expr::InSubquery(expr) => self.lift_in_subquery(&expr.expr, &expr.query),
            Expr::Intersects(expr) => self.simplify_expr_intersects(expr),
            Expr::IsSuperset(expr) => self.simplify_expr_is_superset(expr),
            Expr::Let(expr) => self.simplify_expr_let(expr),
            Expr::List(expr) => self.simplify_expr_list(expr),
            Expr::Map(_) => self.simplify_expr_map(i),
            Expr::Or(expr) => self.simplify_expr_or(expr),
            Expr::IsNull(expr) => self.simplify_expr_is_null(expr),
            Expr::Project(expr) => self.simplify_expr_project(expr),
            _ => None,
        };

        if let Some(mut expr) = maybe_expr {
            // Heavyweight rules may emit new fold-eligible structure
            // (e.g., match elimination produces ANDs containing constants
            // that need short-circuiting).
            fold::fold_stmt(&mut expr);
            *i = expr;
        }
    }

    fn visit_expr_match_mut(&mut self, i: &mut stmt::ExprMatch) {
        // Simplify the subject first.
        self.visit_expr_mut(&mut i.subject);

        // If the subject simplified to a constant, only simplify the matching arm.
        // Skipping dead-code arms avoids panics on expressions like
        // `project([1], Record([I64(disc)]))` that would be invalid to evaluate.
        if let Expr::Value(ref value) = *i.subject {
            let value = value.clone();
            for arm in &mut i.arms {
                if arm.pattern == value {
                    self.visit_expr_mut(&mut arm.expr);
                    return;
                }
            }
        } else {
            for arm in &mut i.arms {
                self.visit_expr_mut(&mut arm.expr);
            }
        }
    }

    fn visit_expr_set_mut(&mut self, i: &mut stmt::ExprSet) {
        match i {
            stmt::ExprSet::SetOp(expr_set_op) if expr_set_op.operands.is_empty() => {
                todo!("is there anything we do here?");
            }
            stmt::ExprSet::SetOp(expr_set_op) if expr_set_op.operands.len() == 1 => {
                let operand = expr_set_op.operands.drain(..).next().unwrap();
                *i = operand;
            }
            stmt::ExprSet::SetOp(expr_set_op) if expr_set_op.is_union() => {
                // First, simplify each sub-query in the union, then rewrite the
                // query as a single disjuntive query.
                let mut operands = vec![];

                Self::flatten_nested_unions(expr_set_op, &mut operands);

                expr_set_op.operands = operands;
            }
            _ => {}
        }

        stmt::visit_mut::visit_expr_set_mut(self, i);
    }

    fn visit_stmt_delete_mut(&mut self, stmt: &mut stmt::Delete) {
        // Visit and simplify source first before pushing a new scope
        self.visit_source_mut(&mut stmt.from);

        let mut s = self.scope(&stmt.from);

        s.visit_filter_mut(&mut stmt.filter);

        if let Some(returning) = &mut stmt.returning {
            s.visit_returning_mut(returning);
        }
    }

    fn visit_stmt_insert_mut(&mut self, stmt: &mut stmt::Insert) {
        // Visit target first before pushing a new scope.
        self.visit_insert_target_mut(&mut stmt.target);

        // Create a new scope for the insert target
        let mut s = self.scope(&stmt.target);

        // First, simplify the source
        s.visit_stmt_query_mut(&mut stmt.source);

        if let Some(returning) = &mut stmt.returning {
            s.visit_returning_mut(returning);
        }
    }

    fn visit_stmt_query_mut(&mut self, stmt: &mut stmt::Query) {
        stmt::visit_mut::visit_stmt_query_mut(self, stmt);

        self.simplify_stmt_query_when_empty(stmt);
    }

    fn visit_stmt_select_mut(&mut self, stmt: &mut stmt::Select) {
        if let stmt::Source::Model(model) = &mut stmt.source
            && let Some(via) = model.via.take()
        {
            todo!("via={via:#?}");
        }

        // Simplify the source first
        self.visit_source_mut(&mut stmt.source);

        // Create a new scope for the insert target
        let mut s = self.scope(&stmt.source);

        s.visit_filter_mut(&mut stmt.filter);
        s.visit_returning_mut(&mut stmt.returning);
    }

    fn visit_stmt_update_mut(&mut self, stmt: &mut stmt::Update) {
        // If the update target is a query, start by simplifying the query, then
        // rewriting it to be a filter.
        if let stmt::UpdateTarget::Query(query) = &mut stmt.target {
            self.visit_stmt_query_mut(query);

            let stmt::ExprSet::Select(select) = &mut query.body else {
                todo!()
            };

            assert!(select.returning.is_model());

            stmt.filter.add_filter(select.filter.take());

            stmt.target = stmt::UpdateTarget::Model(select.source.model_id_unwrap());
        }

        self.visit_update_target_mut(&mut stmt.target);

        let mut s = self.scope(&stmt.target);
        s.visit_assignments_mut(&mut stmt.assignments);

        s.visit_filter_mut(&mut stmt.filter);

        if let Some(expr) = &mut stmt.condition.expr {
            s.visit_expr_mut(expr);
        }

        if let Some(returning) = &mut stmt.returning {
            s.visit_returning_mut(returning);
        }
    }
}

impl<'a> Simplify<'a> {
    pub(crate) fn new(schema: &'a Schema, capability: &'a Capability) -> Self {
        Simplify::with_context(stmt::ExprContext::new(schema), capability)
    }

    pub(crate) fn with_context(cx: stmt::ExprContext<'a>, capability: &'a Capability) -> Self {
        Simplify { cx, capability }
    }

    fn schema(&self) -> &'a Schema {
        self.cx.schema()
    }

    /// Return a new `Simplify` instance that operates on a nested scope
    /// targeting the provided relation.
    pub(crate) fn scope<'scope>(
        &'scope self,
        target: impl IntoExprTarget<'scope>,
    ) -> Simplify<'scope> {
        Simplify {
            cx: self.cx.scope(target),
            capability: self.capability,
        }
    }

    /// Returns the source model
    fn flatten_nested_unions(expr_set_op: &mut stmt::ExprSetOp, operands: &mut Vec<stmt::ExprSet>) {
        assert!(expr_set_op.is_union());

        for expr_set in &mut expr_set_op.operands {
            match expr_set {
                stmt::ExprSet::SetOp(nested_set_op) if nested_set_op.is_union() => {
                    Self::flatten_nested_unions(nested_set_op, operands)
                }
                // Just drop empty values
                stmt::ExprSet::Values(values) if values.is_empty() => {}
                stmt::ExprSet::Select(select) => {
                    if let Some(stmt::ExprSet::Select(tail)) = operands.last_mut() {
                        todo!("merge select={:#?} tail={:#?}", select, tail);
                    }

                    operands.push(std::mem::take(expr_set));
                }
                stmt::ExprSet::Values(values) => {
                    if let Some(stmt::ExprSet::Values(tail)) = operands.last_mut() {
                        tail.rows.append(&mut values.rows);
                        continue;
                    }

                    operands.push(std::mem::take(expr_set));
                }
                _ => todo!("expr={:#?}", expr_set),
            }
        }
    }
}

#[cfg(test)]
mod tests;
