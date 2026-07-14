use crate::engine::fold::{self, expr_let::fold_expr_let};
use toasty_core::stmt::{self, Expr, ExprLet, MatchArm, Value};

// --- fold_expr_let unit tests ---

#[test]
fn single_binding_inlined() {
    // Let { bindings: [I64(42)], body: Arg(0) } → I64(42)
    let mut expr_let = ExprLet {
        bindings: vec![Expr::from(42i64)],
        body: Box::new(Expr::arg(0)),
    };

    let result = fold_expr_let(&mut expr_let);
    assert_eq!(result, Some(Expr::from(42i64)));
}

#[test]
fn multiple_bindings_inlined() {
    // Let { bindings: [I64(1), I64(2)], body: Record([Arg(1), Arg(0)]) }
    // → Record([I64(2), I64(1)])
    let mut expr_let = ExprLet {
        bindings: vec![Expr::from(1i64), Expr::from(2i64)],
        body: Box::new(Expr::record([Expr::arg(1), Expr::arg(0)])),
    };

    let result = fold_expr_let(&mut expr_let);
    assert_eq!(
        result,
        Some(Expr::record([Expr::from(2i64), Expr::from(1i64)]))
    );
}

#[test]
fn unstable_binding_not_inlined() {
    // Binding contains Default (unstable) → no inlining
    let mut expr_let = ExprLet {
        bindings: vec![Expr::Default],
        body: Box::new(Expr::arg(0)),
    };

    let result = fold_expr_let(&mut expr_let);
    assert!(result.is_none());
}

#[test]
fn body_with_match_inlined() {
    // The real-world pattern: nullable relation projection.
    // Let { bindings: [I64(5)], body: Match(Arg(0), [Null → Null], Arg(0)) }
    // → Match(I64(5), [Null → Null], I64(5))
    let mut expr_let = ExprLet {
        bindings: vec![Expr::from(5i64)],
        body: Box::new(Expr::match_expr(
            Expr::arg(0),
            vec![MatchArm {
                pattern: Value::Null,
                expr: Expr::null(),
            }],
            Expr::arg(0),
        )),
    };

    let result = fold_expr_let(&mut expr_let);
    let expected = Expr::match_expr(
        Expr::from(5i64),
        vec![MatchArm {
            pattern: Value::Null,
            expr: Expr::null(),
        }],
        Expr::from(5i64),
    );
    assert_eq!(result, Some(expected));
}

#[test]
fn outer_arg_nesting_decremented() {
    // Body references both the Let binding (nesting=0) and an outer scope
    // (nesting=1).  After inlining, the outer ref should become nesting=0.
    // Let { bindings: [I64(1)], body: Record([Arg(pos=0,nest=0), Arg(pos=0,nest=1)]) }
    // → Record([I64(1), Arg(pos=0,nest=0)])
    let mut expr_let = ExprLet {
        bindings: vec![Expr::from(1i64)],
        body: Box::new(Expr::record([
            Expr::arg(0),
            Expr::Arg(stmt::ExprArg {
                position: 0,
                nesting: 1,
            }),
        ])),
    };

    let result = fold_expr_let(&mut expr_let);
    assert_eq!(result, Some(Expr::record([Expr::from(1i64), Expr::arg(0)])));
}

// --- end-to-end tests through fold_stmt ---

#[test]
fn let_inlined_through_fold_stmt() {
    // End-to-end: fold_stmt should inline the Let.
    let mut expr = Expr::Let(ExprLet {
        bindings: vec![Expr::from(42i64)],
        body: Box::new(Expr::arg(0)),
    });

    fold::fold_stmt(&mut expr);
    assert_eq!(expr, Expr::from(42i64));
}

#[test]
fn nested_let_inlined_bottom_up() {
    // Inner Let is inlined first (bottom-up), then the outer Let.
    // Outer: Let { bindings: [I64(10)], body: Let { bindings: [Arg(0)], body: Arg(0) } }
    // After inner inlining: Let { bindings: [I64(10)], body: Arg(0) }
    // After outer inlining: I64(10)
    let inner = Expr::Let(ExprLet {
        bindings: vec![Expr::arg(0)],
        body: Box::new(Expr::arg(0)),
    });
    let mut expr = Expr::Let(ExprLet {
        bindings: vec![Expr::from(10i64)],
        body: Box::new(inner),
    });

    fold::fold_stmt(&mut expr);
    assert_eq!(expr, Expr::from(10i64));
}

#[test]
fn let_with_match_fully_simplified() {
    // Let { bindings: [Null], body: Match(Arg(0), [Null→Null], Arg(0)) }
    // Step 1 (Let inlining): Match(Null, [Null→Null], Null)
    // Step 2 (Match folding): Null
    //
    // The fold visitor recurses on the replacement after a rule fires
    // (`fold.rs::visit_expr_mut`), so the new Match — now with a constant
    // subject — gets a second pass that folds it to Null.
    let mut expr = Expr::Let(ExprLet {
        bindings: vec![Expr::null()],
        body: Box::new(Expr::match_expr(
            Expr::arg(0),
            vec![MatchArm {
                pattern: Value::Null,
                expr: Expr::null(),
            }],
            Expr::arg(0),
        )),
    });

    fold::fold_stmt(&mut expr);
    assert_eq!(expr, Expr::null());
}
