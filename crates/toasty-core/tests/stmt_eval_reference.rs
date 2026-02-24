use toasty_core::stmt::{ConstInput, Expr, ExprReference, Input, Projection, Value};

/// A simple Input that always resolves any reference to a fixed value.
struct FixedRefInput(Value);

impl Input for FixedRefInput {
    fn resolve_ref(&mut self, _: &ExprReference, _: &Projection) -> Option<Expr> {
        Some(Expr::Value(self.0.clone()))
    }
}

/// An Input that resolves references by field index into a slice of values.
struct SliceRefInput<'a>(&'a [Value]);

impl Input for SliceRefInput<'_> {
    fn resolve_ref(&mut self, expr_ref: &ExprReference, _: &Projection) -> Option<Expr> {
        match expr_ref {
            ExprReference::Field { index, .. } => {
                self.0.get(*index).map(|v| Expr::Value(v.clone()))
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// eval_const — ConstInput cannot resolve references → Err
// ---------------------------------------------------------------------------

#[test]
fn reference_eval_const_is_error() {
    let expr = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 0,
    });
    assert!(expr.eval_const().is_err());
}

#[test]
fn reference_with_const_input_is_error() {
    let expr = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 0,
    });
    assert!(expr.eval(ConstInput::new()).is_err());
}

// ---------------------------------------------------------------------------
// eval() with FixedRefInput — always resolves to the same value
// ---------------------------------------------------------------------------

#[test]
fn reference_fixed_i64() {
    let expr = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 0,
    });
    assert_eq!(
        expr.eval(FixedRefInput(Value::I64(42))).unwrap(),
        Value::I64(42)
    );
}

#[test]
fn reference_fixed_string() {
    let expr = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 0,
    });
    assert_eq!(
        expr.eval(FixedRefInput(Value::from("hello"))).unwrap(),
        Value::from("hello")
    );
}

#[test]
fn reference_fixed_null() {
    let expr = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 0,
    });
    assert_eq!(expr.eval(FixedRefInput(Value::Null)).unwrap(), Value::Null);
}

// ---------------------------------------------------------------------------
// eval() with SliceRefInput — resolves by field index
// ---------------------------------------------------------------------------

#[test]
fn reference_field_index_0() {
    let values = [Value::I64(10), Value::from("hi")];
    let expr = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 0,
    });
    assert_eq!(expr.eval(SliceRefInput(&values)).unwrap(), Value::I64(10));
}

#[test]
fn reference_field_index_1() {
    let values = [Value::I64(10), Value::from("hi")];
    let expr = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 1,
    });
    assert_eq!(
        expr.eval(SliceRefInput(&values)).unwrap(),
        Value::from("hi")
    );
}

// ---------------------------------------------------------------------------
// Reference inside a larger expression
// ---------------------------------------------------------------------------

#[test]
fn reference_inside_is_null() {
    let expr = Expr::is_null(Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 0,
    }));
    assert_eq!(
        expr.eval(FixedRefInput(Value::Null)).unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn reference_inside_binary_op() {
    use toasty_core::stmt::BinaryOp;
    let expr = Expr::binary_op(
        Expr::Reference(ExprReference::Field {
            nesting: 0,
            index: 0,
        }),
        BinaryOp::Eq,
        42i64,
    );
    assert_eq!(
        expr.eval(FixedRefInput(Value::I64(42))).unwrap(),
        Value::Bool(true)
    );
}
