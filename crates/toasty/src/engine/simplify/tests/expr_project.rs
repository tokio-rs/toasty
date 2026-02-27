use super::test_schema;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{self, Expr, ExprMatch, MatchArm, Projection, Value};

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

#[test]
fn project_into_match_distributes() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // project(Match(arg(0), [1 => Record([arg(0), arg(1)]), 2 => Record([arg(0), arg(2)])],
    //               else: Record([arg(0), Error])), [0])
    // → Match(arg(0), [1 => project(Record([arg(0), arg(1)]), [0]),
    //                   2 => project(Record([arg(0), arg(2)]), [0])],
    //               else: project(Record([arg(0), Error]), [0]))
    let mut expr = stmt::ExprProject {
        base: Box::new(Expr::Match(ExprMatch {
            subject: Box::new(Expr::arg(0)),
            arms: vec![
                MatchArm {
                    pattern: Value::from(1i64),
                    expr: Expr::record([Expr::arg(0), Expr::arg(1)]),
                },
                MatchArm {
                    pattern: Value::from(2i64),
                    expr: Expr::record([Expr::arg(0), Expr::arg(2)]),
                },
            ],
            else_expr: Box::new(Expr::record([Expr::arg(0), Expr::error("unreachable")])),
        })),
        projection: Projection::from(0),
    };

    let result = simplify.simplify_expr_project(&mut expr);

    // The result should be a Match with projected arms and else
    assert!(result.is_some());
    let result = result.unwrap();
    if let Expr::Match(m) = result {
        assert_eq!(m.arms.len(), 2);
        // Each arm should now be project(Record(...), [0])
        assert!(m.arms[0].expr.is_project());
        assert!(m.arms[1].expr.is_project());
        // Else should also be projected
        assert!(m.else_expr.is_project());
    } else {
        panic!("expected Match, got {:?}", result);
    }
}
