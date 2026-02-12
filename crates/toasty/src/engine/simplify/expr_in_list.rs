use super::Simplify;
use toasty_core::stmt::{self, Expr, Value};

impl Simplify<'_> {
    pub(super) fn simplify_expr_in_list(&self, expr: &mut stmt::ExprInList) -> Option<Expr> {
        // `x in ()` → `false`
        if expr.list.is_list_empty() {
            return Some(Expr::Value(Value::Bool(false)));
        }

        // Null propagation, `null in (x, y, z)` → `null`
        if expr.expr.is_value_null() {
            return Some(Expr::null());
        }

        self.rewrite_expr_in_list_when_model(expr);

        // Rewrite single-item lists into equalities
        self.rewrite_expr_in_list_with_single_item(expr)
    }

    fn rewrite_expr_in_list_when_model(&self, expr: &mut stmt::ExprInList) {
        if let stmt::Expr::Key(expr_key) = &mut *expr.expr {
            let model = self.model(expr_key.model);

            let primary_key = model
                .primary_key()
                .expect("IN list on model requires root model with primary key");

            if let [pk_field_id] = &primary_key.fields[..] {
                let pk = self.field(*pk_field_id);

                // Check RHS format
                match &mut *expr.list {
                    stmt::Expr::List(expr_list) => {
                        for expr in &mut expr_list.items {
                            match expr {
                                stmt::Expr::Value(value) => {
                                    assert!(value.is_a(&pk.ty.expect_primitive().ty));
                                }
                                _ => todo!("{expr:#?}"),
                            }
                        }
                    }
                    stmt::Expr::Value(stmt::Value::List(values)) => {
                        for value in values {
                            assert!(value.is_a(&pk.ty.expect_primitive().ty));
                        }
                    }
                    _ => todo!("expr={expr:#?}"),
                }

                *expr.expr = stmt::Expr::ref_self_field(pk.id());
            } else {
                // Composite primary key: replace Key with a record of field
                // references. RHS items are expected to be records matching
                // the composite key structure.
                let pk_refs: Vec<_> = primary_key
                    .fields
                    .iter()
                    .map(|field| stmt::Expr::ref_self_field(field))
                    .collect();
                *expr.expr = stmt::Expr::record_from_vec(pk_refs);
            }
        }
    }

    fn rewrite_expr_in_list_with_single_item(&self, expr: &mut stmt::ExprInList) -> Option<Expr> {
        let rhs = match &mut *expr.list {
            Expr::Value(value) => {
                let values = match value {
                    Value::List(value) => &value[..],
                    _ => todo!("{value:#?}"),
                };

                if values.len() != 1 {
                    return None;
                }

                Expr::Value(values[0].clone())
            }
            Expr::List(expr_list) => {
                if expr_list.items.len() != 1 {
                    return None;
                }

                expr_list.items[0].take()
            }
            Expr::Record(_) => todo!("should not happen"),
            _ => return None,
        };

        Some(Expr::eq(expr.expr.take(), rhs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as toasty;
    use crate::engine::simplify::test::{test_schema, test_schema_with};
    use crate::model::Register;

    /// Helper to construct an "in list" expression.
    fn in_list(lhs: Expr, list: Expr) -> stmt::ExprInList {
        stmt::ExprInList {
            expr: Box::new(lhs),
            list: Box::new(list),
        }
    }

    /// Helper for making a value list.
    fn value_list(values: Vec<Value>) -> Expr {
        Expr::Value(Value::List(values))
    }

    /// Helper for making an expression list.
    fn expr_list(items: Vec<Expr>) -> Expr {
        Expr::List(stmt::ExprList { items })
    }

    #[test]
    fn empty_value_list_becomes_false() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `in_list(arg(0), []) → false`
        let mut expr = in_list(Expr::arg(0), value_list(vec![]));
        let result = simplify.simplify_expr_in_list(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_false());
    }

    #[test]
    fn empty_expr_list_becomes_false() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `in_list(arg(0), list([])) → false`
        let mut expr = in_list(Expr::arg(0), expr_list(vec![]));
        let result = simplify.simplify_expr_in_list(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_false());
    }

    #[test]
    fn single_value_becomes_eq() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `in_list(arg(0), [42]) → eq(arg(0), 42)`
        let mut expr = in_list(Expr::arg(0), value_list(vec![Value::from(42i64)]));
        let result = simplify.simplify_expr_in_list(&mut expr);

        assert!(matches!(
            result,
            Some(Expr::BinaryOp(stmt::ExprBinaryOp { lhs, op: stmt::BinaryOp::Eq, rhs }))
                if *lhs == Expr::arg(0) && *rhs == Expr::Value(Value::from(42i64))
        ));
    }

    #[test]
    fn single_expr_becomes_eq() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `in_list(arg(0), list([arg(1)])) → eq(arg(0), arg(1))`
        let mut expr = in_list(Expr::arg(0), expr_list(vec![Expr::arg(1)]));
        let result = simplify.simplify_expr_in_list(&mut expr);

        assert!(matches!(
            result,
            Some(Expr::BinaryOp(stmt::ExprBinaryOp { lhs, op: stmt::BinaryOp::Eq, rhs }))
                if *lhs == Expr::arg(0) && *rhs == Expr::arg(1)
        ));
    }

    #[test]
    fn two_values_unchanged() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `in_list(arg(0), [1, 2])`, multiple items, not simplified
        let mut expr = in_list(
            Expr::arg(0),
            value_list(vec![Value::from(1i64), Value::from(2i64)]),
        );
        let result = simplify.simplify_expr_in_list(&mut expr);

        assert!(result.is_none());
    }

    #[test]
    fn two_exprs_unchanged() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `in_list(arg(0), list([arg(1), arg(2)]))`, multiple items, not simplified
        let mut expr = in_list(Expr::arg(0), expr_list(vec![Expr::arg(1), Expr::arg(2)]));
        let result = simplify.simplify_expr_in_list(&mut expr);

        assert!(result.is_none());
    }

    #[test]
    fn arg_in_single() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `in_list(arg(0), [42]) → eq(arg(0), 42)`
        let mut expr = in_list(Expr::arg(0), value_list(vec![Value::from(42i64)]));
        let result = simplify.simplify_expr_in_list(&mut expr);

        assert!(matches!(
            result,
            Some(Expr::BinaryOp(stmt::ExprBinaryOp { lhs, op: stmt::BinaryOp::Eq, rhs }))
                if *lhs == Expr::arg(0) && *rhs == Expr::Value(Value::from(42i64))
        ));
    }

    #[test]
    fn arg_in_empty() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `in_list(arg(0), []) → false`
        let mut expr = in_list(Expr::arg(0), value_list(vec![]));
        let result = simplify.simplify_expr_in_list(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_false());
    }

    #[test]
    fn arg_in_multi() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `in_list(arg(0), [1, 2])`, multiple items, not simplified
        let mut expr = in_list(
            Expr::arg(0),
            value_list(vec![Value::from(1i64), Value::from(2i64)]),
        );
        let result = simplify.simplify_expr_in_list(&mut expr);

        assert!(result.is_none());
    }

    // Null propagation tests

    #[test]
    fn null_in_list_becomes_null() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `null in (1, 2, 3)` → `null`
        let mut expr = in_list(
            Expr::null(),
            value_list(vec![
                Value::from(1i64),
                Value::from(2i64),
                Value::from(3i64),
            ]),
        );
        let result = simplify.simplify_expr_in_list(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_value_null());
    }

    #[test]
    fn null_in_single_item_becomes_null() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `null in (42)` → `null`
        let mut expr = in_list(Expr::null(), value_list(vec![Value::from(42i64)]));
        let result = simplify.simplify_expr_in_list(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_value_null());
    }

    #[test]
    fn null_in_expr_list_becomes_null() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `null in list([arg(0), arg(1)])` → `null`
        let mut expr = in_list(Expr::null(), expr_list(vec![Expr::arg(0), Expr::arg(1)]));
        let result = simplify.simplify_expr_in_list(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_value_null());
    }

    // Composite key tests

    #[derive(toasty::Model)]
    struct Composite {
        #[key]
        one: String,

        #[key]
        two: String,
    }

    fn composite_schema() -> toasty_core::Schema {
        test_schema_with(&[Composite::schema()])
    }

    #[test]
    fn composite_key_in_list_rewrites_to_record_of_refs() {
        let schema = composite_schema();
        let simplify = Simplify::new(&schema);

        // `key(Composite) IN [("a", "1"), ("b", "2")]`
        // should rewrite Key to `(ref(field0), ref(field1))`
        let list = expr_list(vec![
            Expr::record([
                Expr::Value(Value::from("a")),
                Expr::Value(Value::from("1")),
            ]),
            Expr::record([
                Expr::Value(Value::from("b")),
                Expr::Value(Value::from("2")),
            ]),
        ]);
        let mut expr = in_list(Expr::key(Composite::id()), list);
        let result = simplify.simplify_expr_in_list(&mut expr);

        // Two items, so no single-item rewrite; result is None and the LHS
        // should have been rewritten in place.
        assert!(result.is_none());

        // The LHS should now be a Record of field references
        assert!(
            matches!(&*expr.expr, Expr::Record(r) if r.len() == 2),
            "expected Record with 2 fields, got {:#?}",
            expr.expr,
        );
    }

    #[test]
    fn composite_key_in_single_item_becomes_eq() {
        let schema = composite_schema();
        let simplify = Simplify::new(&schema);

        // `key(Composite) IN [("a", "1")]` → `eq((ref(f0), ref(f1)), ("a", "1"))`
        let list = expr_list(vec![Expr::record([
            Expr::Value(Value::from("a")),
            Expr::Value(Value::from("1")),
        ])]);
        let mut expr = in_list(Expr::key(Composite::id()), list);
        let result = simplify.simplify_expr_in_list(&mut expr);

        // Single item should be rewritten to an equality
        let Some(Expr::BinaryOp(stmt::ExprBinaryOp { op, lhs, .. })) = result else {
            panic!("expected BinaryOp, got {result:#?}");
        };
        assert!(op.is_eq());

        // LHS is the record of field refs
        assert!(
            matches!(lhs.as_ref(), Expr::Record(r) if r.len() == 2),
            "expected Record with 2 fields, got {lhs:#?}",
        );
    }
}
