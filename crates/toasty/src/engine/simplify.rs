mod association;
mod expr_and;
mod expr_binary_op;
mod expr_cast;
mod expr_concat_str;
mod expr_in_list;
mod expr_is_null;
mod expr_list;
mod expr_map;
mod expr_record;

mod value;

// Simplifications
// TODO: unify names
mod lift_in_subquery;
mod lift_pk_select;
mod rewrite_root_path_expr;

use toasty_core::{
    schema::{
        app::{Field, FieldId, Model, ModelId},
        *,
    },
    stmt::{self, Expr, IntoExprTarget, Node, VisitMut},
};

use crate::engine::Engine;

pub(crate) struct Simplify<'a> {
    cx: stmt::ExprContext<'a>,
}

impl Engine {
    pub(crate) fn simplify_stmt<T: Node>(&self, stmt: &mut T) {
        Simplify::new(&self.schema).visit_mut(stmt);
    }
}

// TODO: get rid of this?
pub(crate) fn simplify_expr(cx: stmt::ExprContext<'_>, expr: &mut stmt::Expr) {
    Simplify { cx }.visit_expr_mut(expr);
}

impl VisitMut for Simplify<'_> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        // First, simplify the expression.
        stmt::visit_mut::visit_expr_mut(self, i);

        // If an in-subquery expression, then try lifting it.
        let maybe_expr = match i {
            Expr::Any(expr_any) => match &mut *expr_any.expr {
                Expr::Map(expr_map) if expr_map.base.is_const() => {
                    todo!("simplify; expr={i:#?}")
                }
                _ => None,
            },
            Expr::And(expr_and) => self.simplify_expr_and(expr_and),
            Expr::BinaryOp(expr_binary_op) => self.simplify_expr_binary_op(
                expr_binary_op.op,
                &mut expr_binary_op.lhs,
                &mut expr_binary_op.rhs,
            ),
            Expr::Cast(expr) => self.simplify_expr_cast(expr),
            Expr::ConcatStr(expr) => self.simplify_expr_concat_str(expr),
            Expr::InList(expr) => self.simplify_expr_in_list(expr),
            Expr::InSubquery(expr_in_subquery) => {
                self.lift_in_subquery(&expr_in_subquery.expr, &expr_in_subquery.query)
            }
            Expr::List(expr) => self.simplify_expr_list(expr),
            Expr::Map(_) => self.simplify_expr_map(i),
            Expr::Record(expr) => self.simplify_expr_record(expr),
            Expr::IsNull(expr) => self.simplify_expr_is_null(expr),
            _ => None,
        };

        if let Some(expr) = maybe_expr {
            *i = expr;
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

        // Convert "via" associations into WHERE filters. For example,
        // user.todos().delete(...) becomes "DELETE FROM Todo" with via association,
        // which gets simplified to "DELETE FROM Todo WHERE user_id IN (SELECT id FROM User WHERE ...)"
        self.simplify_via_association_for_delete(stmt);

        let mut s = self.scope(&stmt.from);

        s.visit_filter_mut(&mut stmt.filter);

        if let Some(returning) = &mut stmt.returning {
            s.visit_returning_mut(returning);
        }
    }

    fn visit_stmt_insert_mut(&mut self, stmt: &mut stmt::Insert) {
        // Visit target first before pushing a new scope.
        self.visit_insert_target_mut(&mut stmt.target);

        // Convert "via" associations in insert scopes into WHERE filters. For example,
        // user.todos().insert(...) creates a scope query that gets simplified to ensure
        // inserted todos are automatically linked to the specific user.
        self.simplify_via_association_for_insert(stmt);

        // Create a new scope for the insert target
        let mut s = self.scope(&stmt.target);

        // First, simplify the source
        s.visit_stmt_query_mut(&mut stmt.source);

        if let stmt::ExprSet::Values(values) = &mut stmt.source.body {
            s.normalize_insertion_values(values, stmt.target.width(s.schema()));
        }

        if let Some(returning) = &mut stmt.returning {
            s.visit_returning_mut(returning);
        }
    }

    fn visit_stmt_query_mut(&mut self, stmt: &mut stmt::Query) {
        self.simplify_via_association_for_query(stmt);

        stmt::visit_mut::visit_stmt_query_mut(self, stmt);
    }

    fn visit_stmt_select_mut(&mut self, stmt: &mut stmt::Select) {
        if let stmt::Source::Model(model) = &mut stmt.source {
            if let Some(via) = model.via.take() {
                todo!("via={via:#?}");
            }
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
    pub(crate) fn new(schema: &'a Schema) -> Self {
        Simplify::with_context(stmt::ExprContext::new(schema))
    }

    pub(crate) fn with_context(cx: stmt::ExprContext<'a>) -> Self {
        Simplify { cx }
    }

    fn schema(&self) -> &'a Schema {
        self.cx.schema()
    }

    fn model(&self, model_id: impl Into<ModelId>) -> &Model {
        self.cx.schema().app.model(model_id.into())
    }

    fn field(&self, field_id: impl Into<FieldId>) -> &Field {
        self.cx.schema().app.field(field_id.into())
    }

    /// Return a new `Simplify` instance that operates on a nested scope
    /// targeting the provided relation.
    pub(crate) fn scope<'scope>(
        &'scope self,
        target: impl IntoExprTarget<'scope>,
    ) -> Simplify<'scope> {
        Simplify {
            cx: self.cx.scope(target),
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
                        /*
                        if tail.source == select.source {
                            assert_eq!(select.returning, tail.returning);

                            tail.or(select.filter.take());
                            continue;
                        }
                        */
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

    fn normalize_insertion_values(&mut self, values: &mut stmt::Values, width: usize) {
        for row in &mut values.rows {
            match row {
                stmt::Expr::Record(row) => {
                    assert!(row.len() <= width);

                    while row.len() < width {
                        row.push(stmt::Expr::default());
                    }
                }
                stmt::Expr::Value(stmt::Value::Record(row)) => {
                    assert!(row.len() <= width);

                    while row.len() < width {
                        row.fields.push(stmt::Value::default());
                    }
                }
                _ => todo!("row={row:#?}"),
            }
        }
    }
}
