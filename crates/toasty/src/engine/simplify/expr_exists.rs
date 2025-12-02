use toasty_core::stmt;

use crate::engine::simplify::Simplify;

impl Simplify<'_> {
    pub(super) fn simplify_expr_exists(
        &self,
        expr_exists: &stmt::ExprExists,
    ) -> Option<stmt::Expr> {
        // EXISTS (empty query) -> false
        // NOT EXISTS (empty query) -> true
        if self.stmt_query_is_empty(&expr_exists.subquery) {
            return Some(stmt::Expr::from(expr_exists.negated));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::test::test_schema;
    use toasty_core::stmt::{ExprExists, Query, Values};

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

        // `exists(empty_query) → false`
        let expr = ExprExists {
            subquery: empty_query(),
            negated: false,
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
            negated: false,
        };
        let result = simplify.simplify_expr_exists(&expr);

        assert!(result.is_none());
    }

    #[test]
    fn not_exists_empty_query_becomes_true() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `not_exists(empty_query) → true`
        let expr = ExprExists {
            subquery: empty_query(),
            negated: true,
        };
        let result = simplify.simplify_expr_exists(&expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_true());
    }

    #[test]
    fn not_exists_non_empty_query_not_simplified() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `not_exists(non_empty_query)`, non-empty, not simplified
        let expr = ExprExists {
            subquery: non_empty_query(),
            negated: true,
        };
        let result = simplify.simplify_expr_exists(&expr);

        assert!(result.is_none());
    }
}
