use toasty_core::stmt::{self, Project};

use super::Simplify;

impl Simplify<'_> {
    pub(super) fn simplify_expr_project(
        &mut self,
        expr: &mut stmt::ExprProject,
    ) -> Option<stmt::Expr> {
        // Constant evaluation: if the base is a constant value, we can evaluate
        // the projection at compile time.
        //
        //   - `project(Record([1, 2, 3]), [0])` → `1`
        //   - `project(Record([Record([1, 2]), 3]), [0, 1])` → `2`
        if let stmt::Expr::Value(value) = &*expr.base {
            // Use the value's entry method to follow the projection path
            if let Some(result) = value.project(&expr.projection) {
                return Some(result);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::test::test_schema;
    use toasty_core::stmt::{Expr, Projection, Value};

    #[test]
    fn project_record_single_field() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `project(Record([1, 2, 3]), [0])` → `1`
        let mut expr = stmt::ExprProject {
            base: Box::new(Expr::record([
                Expr::from(1i64),
                Expr::from(2i64),
                Expr::from(3i64),
            ])),
            projection: Projection::from(0),
        };

        let result = simplify.simplify_expr_project(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::I64(1)))));
    }

    #[test]
    fn project_record_second_field() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `project(Record([1, 2, 3]), [1])` → `2`
        let mut expr = stmt::ExprProject {
            base: Box::new(Expr::record([
                Expr::from(1i64),
                Expr::from(2i64),
                Expr::from(3i64),
            ])),
            projection: Projection::from(1),
        };

        let result = simplify.simplify_expr_project(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::I64(2)))));
    }

    #[test]
    fn project_nested_record() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `project(Record([Record([1, 2]), 3]), [0, 1])` → `2`
        let mut expr = stmt::ExprProject {
            base: Box::new(Expr::record([
                Expr::record([Expr::from(1i64), Expr::from(2i64)]),
                Expr::from(3i64),
            ])),
            projection: Projection::from([0, 1]),
        };

        let result = simplify.simplify_expr_project(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::I64(2)))));
    }

    #[test]
    fn project_list_single_element() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `project(List([10, 20, 30]), [1])` → `20`
        let mut expr = stmt::ExprProject {
            base: Box::new(Expr::list([
                Expr::from(10i64),
                Expr::from(20i64),
                Expr::from(30i64),
            ])),
            projection: Projection::from(1),
        };

        let result = simplify.simplify_expr_project(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::I64(20)))));
    }

    #[test]
    fn project_record_with_strings() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `project(Record(["foo", "bar", "baz"]), [2])` → `"baz"`
        let mut expr = stmt::ExprProject {
            base: Box::new(Expr::record([
                Expr::from("foo"),
                Expr::from("bar"),
                Expr::from("baz"),
            ])),
            projection: Projection::from(2),
        };

        let result = simplify.simplify_expr_project(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::String(s))) if s == "baz"));
    }

    #[test]
    fn project_deeply_nested_record() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `project(Record([Record([Record([1, 2, 3]), 4]), 5]), [0, 0, 2])` → `3`
        let mut expr = stmt::ExprProject {
            base: Box::new(Expr::record([
                Expr::record([
                    Expr::record([Expr::from(1i64), Expr::from(2i64), Expr::from(3i64)]),
                    Expr::from(4i64),
                ]),
                Expr::from(5i64),
            ])),
            projection: Projection::from([0, 0, 2]),
        };

        let result = simplify.simplify_expr_project(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::I64(3)))));
    }

    #[test]
    fn project_non_constant_not_simplified() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `project(arg(0), [0])` is not simplified (non-constant base)
        let mut expr = stmt::ExprProject {
            base: Box::new(Expr::arg(0)),
            projection: Projection::from(0),
        };

        let result = simplify.simplify_expr_project(&mut expr);

        assert!(result.is_none());
    }

    #[test]
    fn project_mixed_record_and_list() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `project(Record([List([1, 2]), 3]), [0, 1])` → `2`
        let mut expr = stmt::ExprProject {
            base: Box::new(Expr::record([
                Expr::list([Expr::from(1i64), Expr::from(2i64)]),
                Expr::from(3i64),
            ])),
            projection: Projection::from([0, 1]),
        };

        let result = simplify.simplify_expr_project(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::I64(2)))));
    }

    #[test]
    fn project_identity_path() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `project(42, [])` → `42` (identity projection)
        let mut expr = stmt::ExprProject {
            base: Box::new(Expr::from(42i64)),
            projection: Projection::identity(),
        };

        let result = simplify.simplify_expr_project(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::I64(42)))));
    }

    #[test]
    fn project_null_value() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `project(Record([null, 2]), [0])` → `null`
        let mut expr = stmt::ExprProject {
            base: Box::new(Expr::record([Expr::null(), Expr::from(2i64)])),
            projection: Projection::from(0),
        };

        let result = simplify.simplify_expr_project(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::Null))));
    }
}
