use super::Simplify;
use toasty_core::stmt::{self, Expr};

impl Simplify<'_> {
    pub(super) fn simplify_expr_any(&self, expr_any: &stmt::ExprAny) -> Option<Expr> {
        // Only simplify when the inner expression is a Map with a constant base
        let stmt::Expr::Map(expr_map) = &*expr_any.expr else {
            return None;
        };

        if !expr_map.base.is_const() {
            return None;
        }

        let Ok(base) = expr_map.base.eval_const() else {
            return None;
        };
        let stmt::Value::List(items) = base else {
            todo!()
        };

        let mut operands = vec![];

        for item in items {
            let mut operand = (*expr_map.map).clone();

            match item {
                stmt::Value::Record(value_record) => operand.substitute(&value_record.fields[..]),
                item => operand.substitute([item]),
            }

            operands.push(operand);
        }

        Some(Expr::or_from_vec(operands))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::test::test_schema;
    use toasty_core::stmt::{ExprAny, ExprMap, ExprOr, Value};

    /// Helper to construct an "any of" expression.
    fn any_of(expr: Expr) -> ExprAny {
        ExprAny {
            expr: Box::new(expr),
        }
    }

    /// Helper for making a map expression.
    fn map_expr(base: Expr, map: Expr) -> Expr {
        Expr::Map(ExprMap {
            base: Box::new(base),
            map: Box::new(map),
        })
    }

    /// Helper for making a value list.
    fn value_list(values: Vec<Value>) -> Expr {
        Expr::Value(Value::List(values))
    }

    #[test]
    fn non_map_returns_none() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        let expr = any_of(Expr::arg(0));
        let result = simplify.simplify_expr_any(&expr);

        assert!(result.is_none());
    }

    #[test]
    fn non_const_base_returns_none() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // Map with non-constant base (arg(0))
        let expr = any_of(map_expr(Expr::arg(0), Expr::arg(0)));
        let result = simplify.simplify_expr_any(&expr);

        assert!(result.is_none());
    }

    #[test]
    fn empty_const_list_becomes_false() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `any(map([], x => x)) → false`
        let expr = any_of(map_expr(value_list(vec![]), Expr::arg(0)));
        let result = simplify.simplify_expr_any(&expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_false());
    }

    #[test]
    fn single_item_unwrapped() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `any(map([42], x => x)) → 42`
        let expr = any_of(map_expr(value_list(vec![Value::from(42i64)]), Expr::arg(0)));
        let result = simplify.simplify_expr_any(&expr);

        assert!(matches!(result, Some(Expr::Value(Value::I64(42)))));
    }

    #[test]
    fn multiple_items_become_or() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `any(map([1, 2], x => x)) → or(1, 2)`
        let expr = any_of(map_expr(
            value_list(vec![Value::from(1i64), Value::from(2i64)]),
            Expr::arg(0),
        ));
        let result = simplify.simplify_expr_any(&expr);

        assert!(matches!(
            result,
            Some(Expr::Or(ExprOr { operands }))
                if operands.len() == 2
                    && operands[0] == Expr::Value(Value::from(1i64))
                    && operands[1] == Expr::Value(Value::from(2i64))
        ));
    }
}
