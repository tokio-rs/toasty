use super::Simplify;
use toasty_core::stmt;

impl Simplify<'_> {
    pub(super) fn simplify_expr_list(&mut self, expr: &mut stmt::ExprList) -> Option<stmt::Expr> {
        if let Some(expr) = self.simplify_expr_list_all_values(expr) {
            return Some(expr);
        }

        if let Some(expr) = self.simplify_expr_list_insert_stmt(expr) {
            return Some(expr);
        }

        None
    }

    fn simplify_expr_list_all_values(&mut self, expr: &mut stmt::ExprList) -> Option<stmt::Expr> {
        // If all items are values,
        let all_values = expr.items.iter().all(|expr| expr.is_value());

        if all_values {
            let mut values = vec![];

            for expr in expr.items.drain(..) {
                let stmt::Expr::Value(value) = expr else {
                    panic!()
                };
                values.push(value);
            }

            Some(stmt::Value::list_from_vec(values).into())
        } else {
            None
        }
    }

    // TODO: rewrite this
    fn simplify_expr_list_insert_stmt(&mut self, expr: &mut stmt::ExprList) -> Option<stmt::Expr> {
        // Check if all items are Expr::Stmt with single-row Insert statements
        for item in &expr.items {
            let stmt::Expr::Stmt(expr_stmt) = item else {
                return None;
            };

            let insert = expr_stmt.stmt.as_insert()?;

            // Must be single-row insert
            if !insert.source.single {
                return None;
            }

            // Must have Returning::Model
            match &insert.returning {
                Some(stmt::Returning::Model { .. }) => {}
                _ => return None,
            }

            // Must target a Model (not Table or Scope)
            if !insert.target.is_model() {
                return None;
            }
        }

        // Extract the first insert to get the target model and returning clause
        let first_insert = match &expr.items[0] {
            stmt::Expr::Stmt(s) => s.stmt.as_insert().unwrap(),
            _ => unreachable!(),
        };

        let first_target_model = match &first_insert.target {
            stmt::InsertTarget::Model(model_id) => model_id,
            _ => unreachable!(),
        };

        let first_returning = first_insert.returning.as_ref().unwrap();

        // Check all inserts target the same model and have the same returning clause
        for item in &expr.items[1..] {
            let insert = match item {
                stmt::Expr::Stmt(s) => s.stmt.as_insert().unwrap(),
                _ => unreachable!(),
            };

            let target_model = match &insert.target {
                stmt::InsertTarget::Model(model_id) => model_id,
                _ => unreachable!(),
            };

            if target_model != first_target_model {
                return None;
            }

            if insert.returning.as_ref().unwrap() != first_returning {
                return None;
            }
        }

        // All inserts are compatible, merge them into a single batch insert
        let mut items = expr.items.drain(..).collect::<Vec<_>>();
        let mut merged_insert = match items.remove(0) {
            stmt::Expr::Stmt(s) => s.stmt.unwrap_insert(),
            _ => unreachable!(),
        };

        for item in items {
            let insert = match item {
                stmt::Expr::Stmt(s) => s.stmt.unwrap_insert(),
                _ => unreachable!(),
            };
            merged_insert.merge(insert);
        }

        // Set single = false since we're now returning a list of records
        merged_insert.source.single = false;

        Some(stmt::Expr::stmt(stmt::Statement::Insert(merged_insert)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::test::test_schema;
    use toasty_core::stmt::{Expr, ExprList, Value};

    #[test]
    fn all_const_values_become_value_list() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `list([1, 2, 3]) → [1, 2, 3]`
        let mut expr = ExprList {
            items: vec![
                Expr::Value(Value::from(1i64)),
                Expr::Value(Value::from(2i64)),
                Expr::Value(Value::from(3i64)),
            ],
        };
        let result = simplify.simplify_expr_list(&mut expr);

        assert!(result.is_some());
        let Expr::Value(Value::List(list)) = result.unwrap() else {
            panic!("expected result to be a `Value::List`");
        };
        assert_eq!(list.len(), 3);
        assert_eq!(list[0], Value::from(1i64));
        assert_eq!(list[1], Value::from(2i64));
        assert_eq!(list[2], Value::from(3i64));
    }

    #[test]
    fn mixed_types_in_list() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `list(["hello", 42, true]) → ["hello", 42, true]`
        let mut expr = ExprList {
            items: vec![
                Expr::Value(Value::from("hello")),
                Expr::Value(Value::from(42i64)),
                Expr::Value(Value::from(true)),
            ],
        };
        let result = simplify.simplify_expr_list(&mut expr);

        assert!(result.is_some());
        let Expr::Value(Value::List(list)) = result.unwrap() else {
            panic!("expected result to be a `Value::List`");
        };
        assert_eq!(list.len(), 3);
        assert_eq!(list[0], Value::from("hello"));
        assert_eq!(list[1], Value::from(42i64));
        assert_eq!(list[2], Value::from(true));
    }

    #[test]
    fn empty_list_simplified() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `list([]) → []`
        let mut expr = ExprList { items: vec![] };
        let result = simplify.simplify_expr_list(&mut expr);

        assert!(result.is_some());
        let Expr::Value(Value::List(list)) = result.unwrap() else {
            panic!("expected result to be a `Value::List`");
        };
        assert!(list.is_empty());
    }

    #[test]
    fn non_const_not_simplified() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `list([1, arg(0)])`, non-constant, not simplified
        let mut expr = ExprList {
            items: vec![Expr::Value(Value::from(1i64)), Expr::arg(0)],
        };
        let result = simplify.simplify_expr_list(&mut expr);

        assert!(result.is_none());
    }
}
