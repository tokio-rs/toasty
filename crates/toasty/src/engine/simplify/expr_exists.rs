use toasty_core::stmt;

use crate::engine::simplify::Simplify;

impl Simplify<'_> {
    pub(super) fn simplify_expr_exists(
        &self,
        expr_exists: &stmt::ExprExists,
    ) -> Option<stmt::Expr> {
        // `exists(empty_query)` → `false`
        if self.stmt_query_is_empty(&expr_exists.subquery) {
            return Some(stmt::Expr::FALSE);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::test::test_schema;
    use toasty_core::stmt::{Expr, ExprExists, Query, Values, VisitMut as _};

    /// Creates a query with no rows (empty VALUES clause).
    fn empty_query() -> Box<Query> {
        Box::new(Query::values(Values::default()))
    }

    /// Creates a query with one row containing a single value.
    fn non_empty_query() -> Box<Query> {
        let mut values = Values::default();
        values.rows.push(stmt::Expr::Value(stmt::Value::from(1i64)));
        Box::new(Query::values(values))
    }

    #[test]
    fn exists_empty_query_becomes_false() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `exists(empty_query)` → `false`
        let expr = ExprExists {
            subquery: empty_query(),
        };
        let result = simplify.simplify_expr_exists(&expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_false());
    }

    #[test]
    fn exists_non_empty_query_not_simplified() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `exists(non_empty_query)`, non-empty, not simplified
        let expr = ExprExists {
            subquery: non_empty_query(),
        };
        let result = simplify.simplify_expr_exists(&expr);

        assert!(result.is_none());
    }

    #[test]
    fn not_exists_empty_query_becomes_true() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `not(exists(empty_query))` → `not(false)` → `true`
        let mut expr = Expr::not(Expr::Exists(ExprExists {
            subquery: empty_query(),
        }));
        simplify.visit_expr_mut(&mut expr);
        assert!(expr.is_true());
    }

    #[test]
    fn not_exists_non_empty_query_not_simplified() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `not(exists(non_empty_query))`, non-empty, not simplified
        let mut expr = Expr::not(Expr::Exists(ExprExists {
            subquery: non_empty_query(),
        }));
        simplify.visit_expr_mut(&mut expr);

        // Should remain as `not(exists(...))`
        assert!(matches!(expr, Expr::Not(_)));
    }
}
