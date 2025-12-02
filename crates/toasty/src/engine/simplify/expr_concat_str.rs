use super::Simplify;
use toasty_core::stmt;

impl Simplify<'_> {
    pub(super) fn simplify_expr_concat_str(
        &self,
        expr: &mut stmt::ExprConcatStr,
    ) -> Option<stmt::Expr> {
        if expr.exprs.iter().all(|expr| expr.is_value()) {
            let mut ret = String::new();

            for expr in &expr.exprs {
                let stmt::Expr::Value(value) = expr else {
                    todo!()
                };

                match value {
                    stmt::Value::String(s) => ret.push_str(s),
                    _ => todo!("value={value:#?}"),
                }
            }

            Some(stmt::Expr::Value(ret.into()))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::test::test_schema;
    use toasty_core::stmt::{Expr, ExprConcatStr, Value};

    #[test]
    fn all_const_strings_concatenated() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `concat("hello", " ", "world") → "hello world"`
        let mut expr = ExprConcatStr {
            exprs: vec![
                Expr::Value(Value::from("hello")),
                Expr::Value(Value::from(" ")),
                Expr::Value(Value::from("world")),
            ],
        };
        let result = simplify.simplify_expr_concat_str(&mut expr);

        assert!(result.is_some());
        let Expr::Value(Value::String(s)) = result.unwrap() else {
            panic!("expected result to be a `Value::String`");
        };
        assert_eq!(s, "hello world");
    }

    #[test]
    fn single_const_string() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `concat("alone") → "alone"`
        let mut expr = ExprConcatStr {
            exprs: vec![Expr::Value(Value::from("alone"))],
        };
        let result = simplify.simplify_expr_concat_str(&mut expr);

        assert!(result.is_some());
        let Expr::Value(Value::String(s)) = result.unwrap() else {
            panic!("expected result to be a `Value::String`");
        };
        assert_eq!(s, "alone");
    }

    #[test]
    fn empty_strings_concatenated() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `concat("", "middle", "") → "middle"`
        let mut expr = ExprConcatStr {
            exprs: vec![
                Expr::Value(Value::from("")),
                Expr::Value(Value::from("middle")),
                Expr::Value(Value::from("")),
            ],
        };
        let result = simplify.simplify_expr_concat_str(&mut expr);

        assert!(result.is_some());
        let Expr::Value(Value::String(s)) = result.unwrap() else {
            panic!("expected result to be a `Value::String`");
        };
        assert_eq!(s, "middle");
    }

    #[test]
    fn non_const_not_simplified() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `concat("hello", arg(0))`, non-constant, not simplified
        let mut expr = ExprConcatStr {
            exprs: vec![Expr::Value(Value::from("hello")), Expr::arg(0)],
        };
        let result = simplify.simplify_expr_concat_str(&mut expr);
        assert!(result.is_none());
    }

    #[test]
    fn empty_list_becomes_empty_string() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `concat() → ""`
        let mut expr = ExprConcatStr { exprs: vec![] };
        let result = simplify.simplify_expr_concat_str(&mut expr);

        assert!(result.is_some());
        let Expr::Value(Value::String(s)) = result.unwrap() else {
            panic!("expected result to be a `Value::String`");
        };
        assert_eq!(s, "");
    }
}
