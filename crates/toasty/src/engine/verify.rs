use crate::Result;
use crate::engine::Engine;
use toasty_core::Error;
use toasty_core::driver::Capability;
use toasty_core::{
    schema::{Schema, app::ModelId},
    stmt::{self, Statement, Visit},
};

struct Verify<'a, 'v> {
    schema: &'a Schema,
    capability: &'a Capability,
    error: &'v mut Option<Error>,
}

struct VerifyExpr<'a, 'v> {
    schema: &'a Schema,
    capability: &'a Capability,
    model: ModelId,
    error: &'v mut Option<Error>,
}

impl Engine {
    pub(crate) fn verify(&self, stmt: &Statement) -> Result<()> {
        let mut error = None;
        Verify {
            schema: &self.schema,
            capability: self.capability,
            error: &mut error,
        }
        .visit(stmt);
        match error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }
}

impl stmt::Visit for Verify<'_, '_> {
    fn visit_stmt_delete(&mut self, i: &stmt::Delete) {
        stmt::visit::visit_stmt_delete(self, i);

        VerifyExpr {
            schema: self.schema,
            model: i.from.model_id_unwrap(),
            capability: self.capability,
            error: &mut *self.error,
        }
        .verify_filter(&i.filter);
    }

    fn visit_stmt_query(&mut self, i: &stmt::Query) {
        stmt::visit::visit_stmt_query(self, i);

        self.verify_single_query(i);
        self.verify_offset_key_matches_order_by(i);
        self.verify_limit_is_integer_literal(i);
    }

    fn visit_stmt_select(&mut self, i: &stmt::Select) {
        stmt::visit::visit_stmt_select(self, i);

        VerifyExpr {
            schema: self.schema,
            model: i.source.model_id_unwrap(),
            capability: self.capability,
            error: &mut *self.error,
        }
        .verify_filter(&i.filter);
    }

    fn visit_expr_stmt(&mut self, i: &stmt::ExprStmt) {
        // Mutation sub-statements (delete, update, insert) embedded in
        // expressions must have a returning clause so their result can be
        // used as a value. Query sub-statements produce results implicitly.
        if !i.stmt.is_query() {
            assert!(
                i.stmt.returning().is_some(),
                "mutation sub-statement in expression must have a returning clause; stmt={:#?}",
                i.stmt
            );
        }

        stmt::visit::visit_expr_stmt(self, i);
    }

    fn visit_stmt_update(&mut self, i: &stmt::Update) {
        stmt::visit::visit_stmt_update(self, i);

        // Is not an empty update
        assert!(!i.assignments.is_empty(), "stmt = {i:#?}");

        let mut verify_expr = VerifyExpr {
            schema: self.schema,
            model: i.target.model_id_unwrap(),
            capability: self.capability,
            error: &mut *self.error,
        };

        verify_expr.visit_stmt_update(i);
    }
}

impl Verify<'_, '_> {
    fn verify_offset_key_matches_order_by(&self, i: &stmt::Query) {
        let Some(stmt::Limit::Cursor(cursor)) = i.limit.as_ref() else {
            return;
        };

        let Some(after) = cursor.after.as_ref() else {
            return;
        };

        // SQL requires ORDER BY for cursor-based pagination.
        // NoSQL drivers (DynamoDB) use a driver-level cursor (ExclusiveStartKey)
        // and do not require ORDER BY.
        if !self.capability.sql {
            return;
        }

        let Some(order_by) = i.order_by.as_ref() else {
            todo!("specified offset but no order; stmt={i:#?}");
        };

        match after {
            stmt::Expr::Value(stmt::Value::Record(record)) => {
                if self.capability.sql {
                    assert!(
                        order_by.exprs.len() == record.fields.len(),
                        "order_by = {order_by:#?}"
                    );
                }
                // DDB requires a Record, but the columns counts do not match.
                // The value is a full key, but the order by clause is just the sort key.
            }
            stmt::Expr::Value(_) => {
                if self.capability.sql {
                    assert!(order_by.exprs.len() == 1, "order_by = {order_by:#?}");
                } else {
                    panic!("NoSQL requires a Record as offset");
                }
            }
            _ => todo!("unsupported offset expression; stmt={i:#?}"),
        }
    }

    fn verify_single_query(&self, i: &stmt::Query) {
        if !i.single {
            return;
        }

        if let stmt::ExprSet::Values(values) = &i.body {
            assert_eq!(1, values.rows.len(), "stmt={i:#?}");
        }
    }

    /// Assert that every field inside a `LIMIT` clause is a `Value::I64` literal.
    ///
    /// Builders always normalize integer limits to `I64`, and downstream
    /// consumers (e.g. `extract_query_pk_limit`) rely on this invariant. Any
    /// other variant here means either a builder regressed or the AST was
    /// hand-constructed with a non-canonical shape — both bugs we want to catch
    /// loudly instead of silently degrading to an unbounded scan.
    fn verify_limit_is_integer_literal(&self, i: &stmt::Query) {
        let Some(limit) = i.limit.as_ref() else {
            return;
        };
        match limit {
            stmt::Limit::Cursor(c) => {
                assert_i64_literal(&c.page_size, "Cursor page_size");
            }
            stmt::Limit::Offset(o) => {
                assert_i64_literal(&o.limit, "Offset limit");
                if let Some(off) = o.offset.as_ref() {
                    assert_i64_literal(off, "Offset offset");
                }
            }
        }
    }
}

#[track_caller]
fn assert_i64_literal(expr: &stmt::Expr, what: &str) {
    assert!(
        matches!(expr, stmt::Expr::Value(stmt::Value::I64(_))),
        "{what} must be a Value::I64 literal; got {expr:#?}"
    );
}

impl VerifyExpr<'_, '_> {
    fn verify_filter(&mut self, filter: &stmt::Filter) {
        self.assert_bool_expr(filter.as_expr());
        self.visit_expr(filter.as_expr());
    }

    fn record(&mut self, err: Error) {
        if self.error.is_none() {
            *self.error = Some(err);
        }
    }

    fn assert_bool_expr(&self, expr: &stmt::Expr) {
        use stmt::Expr::*;

        match expr {
            And(_)
            | AllOp(_)
            | AnyOp(_)
            | BinaryOp(_)
            | Like(_)
            | InList(_)
            | InSubquery(_)
            | Intersects(_)
            | IsNull(_)
            | IsSuperset(_)
            | IsVariant(_)
            | Not(_)
            | Or(_)
            | StartsWith(_)
            | Value(stmt::Value::Bool(_)) => {}
            expr => panic!("Not a bool? {expr:#?}"),
        }
    }
}

impl stmt::Visit for VerifyExpr<'_, '_> {
    fn visit_expr_and(&mut self, i: &stmt::ExprAnd) {
        stmt::visit::visit_expr_and(self, i);

        for expr in &i.operands {
            self.assert_bool_expr(expr);
        }
    }

    fn visit_expr_not(&mut self, i: &stmt::ExprNot) {
        stmt::visit::visit_expr_not(self, i);
        self.assert_bool_expr(&i.expr);
    }

    fn visit_expr_or(&mut self, i: &stmt::ExprOr) {
        stmt::visit::visit_expr_or(self, i);

        for expr in &i.operands {
            self.assert_bool_expr(expr);
        }
    }

    fn visit_projection(&mut self, i: &stmt::Projection) {
        let root = self.schema.app.model(self.model);
        assert!(
            self.schema.app.resolve(root, i).is_some(),
            "invalid projection: {i:?}"
        );
    }

    fn visit_expr_project(&mut self, i: &stmt::ExprProject) {
        // For project expressions where the base is a field reference in the
        // current scope, combine the field index with the project's projection
        // to form the full path, then resolve from the root model.
        if let stmt::Expr::Reference(stmt::ExprReference::Field { nesting: 0, index }) = &*i.base {
            let mut full = stmt::Projection::single(*index);
            for step in &i.projection[..] {
                full.push(*step);
            }
            let root = self.schema.app.model(self.model);
            assert!(
                self.schema.app.resolve(root, &full).is_some(),
                "failed to resolve projection: {full:?}"
            );
        } else {
            // For other base expressions (nested projects, etc.), visit the
            // base but skip projection validation since the projection is
            // relative to the base expression's type.
            self.visit_expr(&i.base);
        }
    }

    fn visit_expr_binary_op(&mut self, i: &stmt::ExprBinaryOp) {
        stmt::visit::visit_expr_binary_op(self, i);
    }

    fn visit_expr_in_subquery(&mut self, i: &stmt::ExprInSubquery) {
        // stmt::visit::visit_expr_in_subquery(self, i);

        // Visit **only** the subquery expression
        self.visit(&*i.expr);

        // The subquery is verified independently, sharing the error slot so
        // failures inside it surface to the caller.
        Verify {
            schema: self.schema,
            capability: self.capability,
            error: &mut *self.error,
        }
        .visit(&*i.query);
    }

    fn visit_expr_is_superset(&mut self, i: &stmt::ExprIsSuperset) {
        if !self.capability.native_array_set_predicates && !rhs_is_concrete_list(&i.rhs) {
            self.record(Error::unsupported_feature(
                "is_superset on this driver requires a literal list on the right-hand side",
            ));
        }
        stmt::visit::visit_expr_is_superset(self, i);
    }

    fn visit_expr_intersects(&mut self, i: &stmt::ExprIntersects) {
        if !self.capability.native_array_set_predicates && !rhs_is_concrete_list(&i.rhs) {
            self.record(Error::unsupported_feature(
                "intersects on this driver requires a literal list on the right-hand side",
            ));
        }
        stmt::visit::visit_expr_intersects(self, i);
    }
}

/// True when the expression is — or will fold to — a `Value::List` of
/// concrete values. Verify runs before the simplifier, so the user's
/// `vec![…]` still appears as an `Expr::List` of `Expr::Value` items;
/// `fold::expr_list` collapses that shape to `Value::List` during
/// lowering, which is what the driver eventually sees.
fn rhs_is_concrete_list(expr: &stmt::Expr) -> bool {
    match expr {
        stmt::Expr::Value(stmt::Value::List(_)) => true,
        stmt::Expr::List(list) => list
            .items
            .iter()
            .all(|item| matches!(item, stmt::Expr::Value(_))),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::test_util::test_schema;
    use toasty_core::driver::Capability;
    use toasty_core::stmt::{Expr, ExprIsSuperset, ExprList, Value};

    fn verify_with(capability: &'static Capability, stmt: Statement) -> Result<()> {
        let schema = test_schema();
        let mut error = None;
        Verify {
            schema: &schema,
            capability,
            error: &mut error,
        }
        .visit(&stmt);
        match error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    fn verify_expr_with(capability: &'static Capability, expr: &Expr) -> Option<Error> {
        let schema = test_schema();
        let mut error = None;
        // ModelId is only used by projection-checking visitor methods, which
        // these expression-only tests don't trigger.
        VerifyExpr {
            schema: &schema,
            capability,
            model: toasty_core::schema::app::ModelId(0),
            error: &mut error,
        }
        .visit_expr(expr);
        error
    }

    fn is_superset(rhs: Expr) -> Expr {
        Expr::IsSuperset(ExprIsSuperset {
            lhs: Box::new(Expr::arg(0)),
            rhs: Box::new(rhs),
        })
    }

    #[test]
    #[should_panic(expected = "Offset offset must be a Value::I64 literal")]
    fn offset_with_non_i64_limit_panics() {
        let mut query = stmt::Query::unit();
        query.limit = Some(stmt::Limit::Offset(stmt::LimitOffset {
            limit: stmt::Value::I64(10).into(),
            offset: Some(stmt::Value::U64(5).into()),
        }));
        verify_with(&Capability::SQLITE, Statement::Query(query)).unwrap();
    }

    #[test]
    fn is_superset_literal_rhs_accepted_on_ddb() {
        let expr = is_superset(Expr::Value(Value::List(vec![Value::I64(1)])));
        assert!(verify_expr_with(&Capability::DYNAMODB, &expr).is_none());
    }

    #[test]
    fn is_superset_pre_fold_expr_list_accepted_on_ddb() {
        // Pre-simplifier shape produced by `is_superset(vec![…])`: an
        // `Expr::List` of `Expr::Value` items. The fold pass will collapse
        // this to `Value::List` during lowering.
        let expr = is_superset(Expr::List(ExprList {
            items: vec![Expr::Value(Value::I64(1)), Expr::Value(Value::I64(2))],
        }));
        assert!(verify_expr_with(&Capability::DYNAMODB, &expr).is_none());
    }

    #[test]
    fn is_superset_non_literal_rhs_rejected_on_ddb() {
        let expr = is_superset(Expr::arg(1));
        let err = verify_expr_with(&Capability::DYNAMODB, &expr)
            .expect("expected unsupported_feature error");
        assert!(err.is_unsupported_feature());
    }

    #[test]
    fn is_superset_non_literal_rhs_accepted_on_sqlite() {
        let expr = is_superset(Expr::arg(1));
        assert!(verify_expr_with(&Capability::SQLITE, &expr).is_none());
    }
}
