use crate::engine::Engine;
use toasty_core::driver::Capability;
use toasty_core::{
    schema::{Schema, app::ModelId},
    stmt::{self, Statement, Visit},
};

struct Verify<'a> {
    schema: &'a Schema,
    capability: &'a Capability,
}

struct VerifyExpr<'a> {
    schema: &'a Schema,
    capability: &'a Capability,
    model: ModelId,
}

impl Engine {
    pub(crate) fn verify(&self, stmt: &Statement) {
        Verify {
            schema: &self.schema,
            capability: self.capability,
        }
        .visit(stmt);
    }
}

impl stmt::Visit for Verify<'_> {
    fn visit_stmt_delete(&mut self, i: &stmt::Delete) {
        stmt::visit::visit_stmt_delete(self, i);

        VerifyExpr {
            schema: self.schema,
            model: i.from.model_id_unwrap(),
            capability: self.capability,
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
        };

        verify_expr.visit_stmt_update(i);
    }
}

impl Verify<'_> {
    fn verify_offset_key_matches_order_by(&self, i: &stmt::Query) {
        let Some(stmt::Limit::Cursor(cursor)) = i.limit.as_ref() else {
            return;
        };

        let Some(after) = cursor.after.as_ref() else {
            return;
        };

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

impl VerifyExpr<'_> {
    fn verify_filter(&mut self, filter: &stmt::Filter) {
        self.assert_bool_expr(filter.as_expr());
        self.visit_expr(filter.as_expr());
    }

    fn assert_bool_expr(&self, expr: &stmt::Expr) {
        use stmt::Expr::*;

        match expr {
            And(_)
            | StartsWith(_)
            | BinaryOp(_)
            | Like(_)
            | InList(_)
            | InSubquery(_)
            | IsNull(_)
            | IsVariant(_)
            | Not(_)
            | Or(_)
            | Value(stmt::Value::Bool(_)) => {}
            expr => panic!("Not a bool? {expr:#?}"),
        }
    }
}

impl stmt::Visit for VerifyExpr<'_> {
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

        // The subquery is verified independently
        Verify {
            schema: self.schema,
            capability: self.capability,
        }
        .visit(&*i.query);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::test_util::test_schema;
    use toasty_core::driver::Capability;

    fn verify_query(query: stmt::Query) {
        let schema = test_schema();
        Verify {
            schema: &schema,
            capability: &Capability::SQLITE,
        }
        .visit(&Statement::Query(query));
    }

    #[test]
    #[should_panic(expected = "Offset offset must be a Value::I64 literal")]
    fn offset_with_non_i64_limit_panics() {
        let mut query = stmt::Query::unit();
        query.limit = Some(stmt::Limit::Offset(stmt::LimitOffset {
            limit: stmt::Value::I64(10).into(),
            offset: Some(stmt::Value::U64(5).into()),
        }));
        verify_query(query);
    }
}
