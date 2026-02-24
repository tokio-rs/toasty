use crate as toasty;
use crate::engine::simplify::Simplify;
use crate::model::Register;
use toasty_core::{
    driver::Capability,
    schema::{app, Builder},
    stmt::{BinaryOp, Expr, ExprCast, ExprReference, Type, Value},
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
        app::Schema::from_macro(&[User::schema()]).expect("schema should build from macro");

    Builder::new()
        .build(app_schema, &Capability::SQLITE)
        .expect("schema should build")
}

#[test]
fn non_id_cast_not_unwrapped() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

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
fn constant_eq_same_values_becomes_true() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `eq(5, 5)` → `true`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_eq_different_values_becomes_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `eq(1, 2)` → `false`
    let mut lhs = Expr::Value(Value::from(1i64));
    let mut rhs = Expr::Value(Value::from(2i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn constant_ne_same_values_becomes_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `ne(5, 5)` → `false`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn constant_ne_different_values_becomes_true() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `ne("abc", "def")` → `true`
    let mut lhs = Expr::Value(Value::from("abc"));
    let mut rhs = Expr::Value(Value::from("def"));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_eq_with_null_becomes_null() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `eq(null, 5)` → `null`
    let mut lhs = Expr::Value(Value::Null);
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Null))));
}

#[test]
fn constant_eq_null_with_null_becomes_null() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `eq(null, null)` → `null`
    let mut lhs = Expr::Value(Value::Null);
    let mut rhs = Expr::Value(Value::Null);

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Null))));
}

#[test]
fn constant_lt_becomes_true() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `1 < 5` → `true`
    let mut lhs = Expr::Value(Value::from(1i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_lt_becomes_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `5 < 1` → `false`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::Value(Value::from(1i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn constant_le_equal_becomes_true() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `5 <= 5` → `true`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Le, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_le_becomes_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `10 <= 5` → `false`
    let mut lhs = Expr::Value(Value::from(10i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Le, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn constant_gt_becomes_true() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `10 > 5` → `true`
    let mut lhs = Expr::Value(Value::from(10i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Gt, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_gt_becomes_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `1 > 5` → `false`
    let mut lhs = Expr::Value(Value::from(1i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Gt, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn constant_ge_equal_becomes_true() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `5 >= 5` → `true`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Ge, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_ge_becomes_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `1 >= 5` → `false`
    let mut lhs = Expr::Value(Value::from(1i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Ge, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn constant_lt_string_lexicographic() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `"abc" < "def"` → `true` (lexicographic)
    let mut lhs = Expr::Value(Value::from("abc"));
    let mut rhs = Expr::Value(Value::from("def"));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_lt_different_types_not_simplified() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `5 < "abc"` is not simplified (incompatible types)
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::Value(Value::from("abc"));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    assert!(result.is_none());
}

#[test]
fn lt_with_non_constant_not_simplified() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `arg(0) < 5` is not simplified (non-constant lhs)
    let mut lhs = Expr::arg(0);
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    assert!(result.is_none());
}

#[test]
fn self_comparison_eq_non_nullable_becomes_true() {
    let schema = test_schema();
    let model = schema.app.model(User::id());
    let simplify = Simplify::new(&schema);
    let mut simplify = simplify.scope(model.expect_root());

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
    let simplify = Simplify::new(&schema);
    let mut simplify = simplify.scope(model.expect_root());

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
    let simplify = Simplify::new(&schema);
    let mut simplify = simplify.scope(model.expect_root());

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
    let simplify = Simplify::new(&schema);
    let mut simplify = simplify.scope(model.expect_root());

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
fn x_eq_true_becomes_x() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `x = true` → `x`
    let mut lhs = Expr::arg(0);
    let mut rhs = Expr::Value(Value::Bool(true));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Arg(_))));
}

#[test]
fn true_eq_x_becomes_x() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `true = x` → `x`
    let mut lhs = Expr::Value(Value::Bool(true));
    let mut rhs = Expr::arg(0);

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Arg(_))));
}

#[test]
fn x_eq_false_becomes_not_x() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `x = false` → `not(x)`
    let mut lhs = Expr::arg(0);
    let mut rhs = Expr::Value(Value::Bool(false));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Not(_))));
}

#[test]
fn x_ne_true_becomes_not_x() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `x != true` → `not(x)`
    let mut lhs = Expr::arg(0);
    let mut rhs = Expr::Value(Value::Bool(true));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Not(_))));
}

#[test]
fn x_ne_false_becomes_x() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `x != false` → `x`
    let mut lhs = Expr::arg(0);
    let mut rhs = Expr::Value(Value::Bool(false));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Arg(_))));
}

#[test]
fn canonicalize_eq_literal_on_left() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `5 = x` → `x = 5`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::arg(0);

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected BinaryOp");
    };
    assert_eq!(binary_op.op, BinaryOp::Eq);
    assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
    assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
}

#[test]
fn canonicalize_lt_literal_on_left() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `5 < x` → `x > 5`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::arg(0);

    let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected BinaryOp");
    };
    assert_eq!(binary_op.op, BinaryOp::Gt);
    assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
    assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
}

#[test]
fn canonicalize_gt_literal_on_left() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `5 > x` → `x < 5`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::arg(0);

    let result = simplify.simplify_expr_binary_op(BinaryOp::Gt, &mut lhs, &mut rhs);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected BinaryOp");
    };
    assert_eq!(binary_op.op, BinaryOp::Lt);
    assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
    assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
}

#[test]
fn canonicalize_le_literal_on_left() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `5 <= x` → `x >= 5`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::arg(0);

    let result = simplify.simplify_expr_binary_op(BinaryOp::Le, &mut lhs, &mut rhs);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected BinaryOp");
    };
    assert_eq!(binary_op.op, BinaryOp::Ge);
    assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
    assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
}

#[test]
fn canonicalize_ge_literal_on_left() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `5 >= x` → `x <= 5`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::arg(0);

    let result = simplify.simplify_expr_binary_op(BinaryOp::Ge, &mut lhs, &mut rhs);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected BinaryOp");
    };
    assert_eq!(binary_op.op, BinaryOp::Le);
    assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
    assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
}

#[test]
fn no_canonicalize_when_literal_on_right() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `x < 5` is already canonical, no change
    let mut lhs = Expr::arg(0);
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = simplify.simplify_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    assert!(result.is_none());
}

#[test]
fn tuple_eq_decomposition_two_elements() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

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
    let mut simplify = Simplify::new(&schema);

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
    let mut simplify = Simplify::new(&schema);

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
    let mut simplify = Simplify::new(&schema);

    // `(a) = (x)` → `a = x`
    let mut lhs = Expr::record([Expr::arg(0)]);
    let mut rhs = Expr::record([Expr::arg(1)]);

    let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);
    assert!(matches!(result, Some(Expr::BinaryOp(op)) if op.op.is_eq()));
}
