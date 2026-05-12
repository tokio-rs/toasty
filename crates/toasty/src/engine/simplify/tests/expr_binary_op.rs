use crate as toasty;
use crate::engine::simplify::Simplify;
use crate::schema::Register;
use toasty_core::{
    driver::Capability,
    schema::{Builder, app},
    stmt::{BinaryOp, Expr, ExprCast, ExprReference, MatchArm, Type, Value, ValueRecord, VisitMut},
};

#[derive(toasty::Model)]
struct User {
    #[key]
    id: String,

    #[allow(dead_code)]
    name: Option<String>,
}

fn test_schema() -> toasty_core::Schema {
    let app_schema =
        app::Schema::from_macro([User::schema()]).expect("schema should build from macro");

    Builder::new()
        .build(app_schema, &Capability::SQLITE)
        .expect("schema should build")
}

#[test]
fn non_id_cast_not_unwrapped() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `eq(cast(arg(0), String), "test")`, non-Id cast is not unwrapped
    let mut lhs = Expr::Cast(ExprCast {
        expr: Box::new(Expr::arg(0)),
        ty: Type::String,
    });
    let mut rhs = Expr::Value(Value::from("test"));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(result.is_none());
    assert!(matches!(lhs, Expr::Cast(_)));
}

#[test]
fn self_comparison_eq_non_nullable_becomes_true() {
    let schema = test_schema();
    let model = schema.app.model(User::id());
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);
    let mut simplify = simplify.scope(model.as_root_unwrap());

    // `id = id` → `true` (non-nullable field)
    let mut lhs = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 0,
    });
    let mut rhs = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 0,
    });

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn self_comparison_ne_non_nullable_becomes_false() {
    let schema = test_schema();
    let model = schema.app.model(User::id());
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);
    let mut simplify = simplify.scope(model.as_root_unwrap());

    // `id != id` → `false` (non-nullable field)
    let mut lhs = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 0,
    });
    let mut rhs = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 0,
    });

    let result = simplify.simplify_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn self_comparison_nullable_not_simplified() {
    let schema = test_schema();
    let model = schema.app.model(User::id());
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);
    let mut simplify = simplify.scope(model.as_root_unwrap());

    // `name = name` is not simplified (nullable field)
    let mut lhs = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 1,
    });
    let mut rhs = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 1,
    });

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(result.is_none());
}

#[test]
fn different_fields_not_simplified() {
    let schema = test_schema();
    let model = schema.app.model(User::id());
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);
    let mut simplify = simplify.scope(model.as_root_unwrap());

    // `id = name` is not simplified (different fields)
    let mut lhs = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 0,
    });
    let mut rhs = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 1,
    });

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(result.is_none());
}

#[test]
fn tuple_eq_decomposition_two_elements() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `(a, b) = (x, y)` → `a = x and b = y`
    let mut lhs = Expr::record([Expr::arg(0), Expr::arg(1)]);
    let mut rhs = Expr::record([Expr::arg(2), Expr::arg(3)]);

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    let Some(Expr::And(and_expr)) = result else {
        panic!("expected And expression");
    };
    assert_eq!(and_expr.len(), 2);
    assert!(matches!(&and_expr[0], Expr::BinaryOp(op) if op.op.is_eq()));
    assert!(matches!(&and_expr[1], Expr::BinaryOp(op) if op.op.is_eq()));
}

#[test]
fn tuple_eq_decomposition_three_elements() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `(a, b, c) = (x, y, z)` → `a = x and b = y and c = z`
    let mut lhs = Expr::record([Expr::arg(0), Expr::arg(1), Expr::arg(2)]);
    let mut rhs = Expr::record([Expr::arg(3), Expr::arg(4), Expr::arg(5)]);

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    let Some(Expr::And(and_expr)) = result else {
        panic!("expected And expression");
    };
    assert_eq!(and_expr.len(), 3);
}

#[test]
fn tuple_ne_decomposition() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `(a, b) != (x, y)` → `a != x or b != y`
    let mut lhs = Expr::record([Expr::arg(0), Expr::arg(1)]);
    let mut rhs = Expr::record([Expr::arg(2), Expr::arg(3)]);

    let result = simplify.simplify_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

    let Some(Expr::Or(or_expr)) = result else {
        panic!("expected Or expression");
    };
    assert_eq!(or_expr.len(), 2);
    assert!(matches!(&or_expr[0], Expr::BinaryOp(op) if op.op.is_ne()));
    assert!(matches!(&or_expr[1], Expr::BinaryOp(op) if op.op.is_ne()));
}

#[test]
fn single_element_tuple_eq() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // `(a) = (x)` → `a = x`
    let mut lhs = Expr::record([Expr::arg(0)]);
    let mut rhs = Expr::record([Expr::arg(1)]);

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);
    assert!(matches!(result, Some(Expr::BinaryOp(op)) if op.op.is_eq()));
}

// --- Match elimination tests ---

#[test]
fn match_eq_constant_value() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // Match(col, [1 => Record([col, addr]), 2 => Record([col, num])],
    //       else: Record([col, Error])) == Value(Record([I64(1), "alice"]))
    // → col == 1 AND addr == "alice"
    //
    // The else branch uses Record([col, Error]) matching the real-world
    // data-carrying enum pattern. Tuple decomposition produces col == I64(1)
    // which contradicts the NOT(col == 1) guard, so the complement law
    // folds the else term to false.
    let mut expr = Expr::binary_op(
        Expr::match_expr(
            Expr::arg(0),
            vec![
                MatchArm {
                    pattern: Value::from(1i64),
                    expr: Expr::record([Expr::arg(0), Expr::arg(1)]),
                },
                MatchArm {
                    pattern: Value::from(2i64),
                    expr: Expr::record([Expr::arg(0), Expr::arg(2)]),
                },
            ],
            Expr::record([Expr::arg(0), Expr::error("unreachable")]),
        ),
        BinaryOp::Eq,
        Expr::from(Value::Record(ValueRecord::from_vec(vec![
            Value::from(1i64),
            Value::from("alice"),
        ]))),
    );

    simplify.visit_expr_mut(&mut expr);

    // Should be: arg(0) == 1 AND arg(1) == "alice"
    let Expr::And(and) = &expr else {
        panic!("expected And, got {expr:?}");
    };
    assert_eq!(and.len(), 2);
}

#[test]
fn match_eq_scalar_folds_matching_arm() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // Match(arg(0), [1 => "a", 2 => "b"], else: "__") == "a" → arg(0) == 1
    // The else value "__" != "a" folds to false, pruning the else term.
    let mut expr = Expr::binary_op(
        Expr::match_expr(
            Expr::arg(0),
            vec![
                MatchArm {
                    pattern: Value::from(1i64),
                    expr: Expr::from("a"),
                },
                MatchArm {
                    pattern: Value::from(2i64),
                    expr: Expr::from("b"),
                },
            ],
            Expr::from("__"),
        ),
        BinaryOp::Eq,
        Expr::from("a"),
    );

    simplify.visit_expr_mut(&mut expr);

    // Only arm 1 survives (arm 2: "b" == "a" → false, pruned)
    // Result: arg(0) == 1
    let Expr::BinaryOp(binop) = &expr else {
        panic!("expected BinaryOp, got {expr:?}");
    };
    assert!(binop.op.is_eq());
    assert!(matches!(*binop.lhs, Expr::Arg(_)));
    assert!(matches!(*binop.rhs, Expr::Value(Value::I64(1))));
}

#[test]
fn match_eq_no_matching_arm_folds_to_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // Match(arg(0), [1 => "a", 2 => "b"], else: "__") == "c" → false (all arms pruned)
    // The else value "__" != "c" folds to false, pruning the else term too.
    let mut expr = Expr::binary_op(
        Expr::match_expr(
            Expr::arg(0),
            vec![
                MatchArm {
                    pattern: Value::from(1i64),
                    expr: Expr::from("a"),
                },
                MatchArm {
                    pattern: Value::from(2i64),
                    expr: Expr::from("b"),
                },
            ],
            Expr::from("__"),
        ),
        BinaryOp::Eq,
        Expr::from("c"),
    );

    simplify.visit_expr_mut(&mut expr);

    assert!(expr.is_false(), "expected false, got {expr:?}");
}

#[test]
fn match_on_rhs() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // "a" == Match(arg(0), [1 => "a", 2 => "b"], else: "__") → arg(0) == 1
    let mut expr = Expr::binary_op(
        Expr::from("a"),
        BinaryOp::Eq,
        Expr::match_expr(
            Expr::arg(0),
            vec![
                MatchArm {
                    pattern: Value::from(1i64),
                    expr: Expr::from("a"),
                },
                MatchArm {
                    pattern: Value::from(2i64),
                    expr: Expr::from("b"),
                },
            ],
            Expr::from("__"),
        ),
    );

    simplify.visit_expr_mut(&mut expr);

    // Only arm 1 survives
    let Expr::BinaryOp(binop) = &expr else {
        panic!("expected BinaryOp, got {expr:?}");
    };
    assert!(binop.op.is_eq());
    assert!(matches!(*binop.lhs, Expr::Arg(_)));
    assert!(matches!(*binop.rhs, Expr::Value(Value::I64(1))));
}

#[test]
fn match_ne_preserves_non_matching_arms() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);

    // Match(arg(0), [1 => "a", 2 => "b"], else: "a") != "a"
    // arm 1: arg(0) == 1 AND "a" != "a" → false → pruned
    // arm 2: arg(0) == 2 AND "b" != "a" → arg(0) == 2
    // else:  NOT(arg(0)==1) AND NOT(arg(0)==2) AND "a" != "a" → false → pruned
    let mut expr = Expr::binary_op(
        Expr::match_expr(
            Expr::arg(0),
            vec![
                MatchArm {
                    pattern: Value::from(1i64),
                    expr: Expr::from("a"),
                },
                MatchArm {
                    pattern: Value::from(2i64),
                    expr: Expr::from("b"),
                },
            ],
            Expr::from("a"),
        ),
        BinaryOp::Ne,
        Expr::from("a"),
    );

    simplify.visit_expr_mut(&mut expr);

    // Only arm 2 survives → arg(0) == 2
    let Expr::BinaryOp(binop) = &expr else {
        panic!("expected BinaryOp, got {expr:?}");
    };
    assert!(binop.op.is_eq());
    assert!(matches!(*binop.lhs, Expr::Arg(_)));
    assert!(matches!(*binop.rhs, Expr::Value(Value::I64(2))));
}

#[test]
fn match_with_non_constant_subject() {
    let schema = test_schema();
    let model = schema.app.model(User::id());
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);
    let mut simplify = simplify.scope(model.as_root_unwrap());

    // Match over a column reference (the real-world case)
    // Match(field[0], [1 => "a", 2 => "b"], else: "__") == "a"
    let subject = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 0,
    });

    let mut expr = Expr::binary_op(
        Expr::match_expr(
            subject,
            vec![
                MatchArm {
                    pattern: Value::from(1i64),
                    expr: Expr::from("a"),
                },
                MatchArm {
                    pattern: Value::from(2i64),
                    expr: Expr::from("b"),
                },
            ],
            Expr::from("__"),
        ),
        BinaryOp::Eq,
        Expr::from("a"),
    );

    simplify.visit_expr_mut(&mut expr);

    // Only arm 1 survives. The guard becomes field[0] == 1.
    // The exact shape depends on canonicalization, but there should be no Match left.
    assert!(
        !matches!(&expr, Expr::Match(_)),
        "Match should be eliminated, got {expr:?}"
    );
}
