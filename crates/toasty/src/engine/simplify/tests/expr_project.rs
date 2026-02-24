use super::test_schema;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{self, Expr, Projection, Value};

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
