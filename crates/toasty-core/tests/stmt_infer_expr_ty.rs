use toasty_core::stmt::{BinaryOp, Expr, ExprAnd, ExprArg, ExprContext, ExprOr, Type, Value};

fn cx() -> ExprContext<'static, ()> {
    ExprContext::new_free()
}

fn infer(expr: &Expr) -> Type {
    cx().infer_expr_ty(expr, &[])
}

fn infer_with_args(expr: &Expr, args: &[Type]) -> Type {
    cx().infer_expr_ty(expr, args)
}

// ---------------------------------------------------------------------------
// Expr::Value — delegates to Value::infer_ty
// ---------------------------------------------------------------------------

#[test]
fn infer_value_bool() {
    assert_eq!(infer(&Expr::from(true)), Type::Bool);
}

#[test]
fn infer_value_i32() {
    assert_eq!(infer(&Expr::from(Value::I32(42))), Type::I32);
}

#[test]
fn infer_value_i64() {
    assert_eq!(infer(&Expr::from(Value::I64(0))), Type::I64);
}

#[test]
fn infer_value_string() {
    assert_eq!(infer(&Expr::from("hello")), Type::String);
}

#[test]
fn infer_value_null() {
    assert_eq!(infer(&Expr::null()), Type::Null);
}

// ---------------------------------------------------------------------------
// Boolean predicates — always Type::Bool
// ---------------------------------------------------------------------------

#[test]
fn infer_and() {
    let expr = ExprAnd {
        operands: vec![true.into(), false.into()],
    }
    .into();
    assert_eq!(infer(&expr), Type::Bool);
}

#[test]
fn infer_or() {
    let expr = ExprOr {
        operands: vec![false.into(), true.into()],
    }
    .into();
    assert_eq!(infer(&expr), Type::Bool);
}

#[test]
fn infer_is_null() {
    assert_eq!(infer(&Expr::is_null(Expr::null())), Type::Bool);
    assert_eq!(infer(&Expr::is_null(Expr::from(Value::I32(1)))), Type::Bool);
}

#[test]
fn infer_binary_op_eq() {
    assert_eq!(
        infer(&Expr::binary_op(1i64, BinaryOp::Eq, 2i64)),
        Type::Bool
    );
}

#[test]
fn infer_binary_op_lt() {
    assert_eq!(
        infer(&Expr::binary_op(1i64, BinaryOp::Lt, 2i64)),
        Type::Bool
    );
}

// ---------------------------------------------------------------------------
// Expr::Cast — returns the declared target type
// ---------------------------------------------------------------------------

#[test]
fn infer_cast_to_i64() {
    let expr = Expr::cast(Expr::from(Value::I32(5)), Type::I64);
    assert_eq!(infer(&expr), Type::I64);
}

#[test]
fn infer_cast_to_string() {
    let expr = Expr::cast(Expr::from(Value::I32(5)), Type::String);
    assert_eq!(infer(&expr), Type::String);
}

#[test]
fn infer_cast_to_bool() {
    let expr = Expr::cast(Expr::null(), Type::Bool);
    assert_eq!(infer(&expr), Type::Bool);
}

// ---------------------------------------------------------------------------
// Expr::List — wraps first element's type in List
// ---------------------------------------------------------------------------

#[test]
fn infer_list_of_i32() {
    let expr = Expr::list([Value::I32(1), Value::I32(2)]);
    assert_eq!(infer(&expr), Type::list(Type::I32));
}

#[test]
fn infer_list_of_string() {
    let expr = Expr::list([Value::String("a".into())]);
    assert_eq!(infer(&expr), Type::list(Type::String));
}

#[test]
fn infer_list_of_bool() {
    let expr = Expr::list([Value::Bool(false), Value::Bool(true)]);
    assert_eq!(infer(&expr), Type::list(Type::Bool));
}

// ---------------------------------------------------------------------------
// Expr::Record — per-field types
// ---------------------------------------------------------------------------

#[test]
fn infer_record_two_fields() {
    let expr = Expr::record([Expr::from(Value::I32(1)), Expr::from("x")]);
    assert_eq!(infer(&expr), Type::Record(vec![Type::I32, Type::String]));
}

#[test]
fn infer_record_single_field() {
    let expr = Expr::record([Expr::from(Value::Bool(true))]);
    assert_eq!(infer(&expr), Type::Record(vec![Type::Bool]));
}

#[test]
fn infer_record_with_null() {
    let expr = Expr::record([Expr::from(Value::U64(9)), Expr::null()]);
    assert_eq!(infer(&expr), Type::Record(vec![Type::U64, Type::Null]));
}

// ---------------------------------------------------------------------------
// Expr::Arg — resolved from the args slice
// ---------------------------------------------------------------------------

#[test]
fn infer_arg_position_0() {
    let expr = Expr::arg(ExprArg::new(0));
    assert_eq!(infer_with_args(&expr, &[Type::I64]), Type::I64);
}

#[test]
fn infer_arg_position_1() {
    let expr = Expr::arg(ExprArg::new(1));
    assert_eq!(
        infer_with_args(&expr, &[Type::Bool, Type::String]),
        Type::String
    );
}

#[test]
fn infer_arg_position_2() {
    let expr = Expr::arg(ExprArg::new(2));
    assert_eq!(
        infer_with_args(&expr, &[Type::I32, Type::Bool, Type::Bytes]),
        Type::Bytes
    );
}

// ---------------------------------------------------------------------------
// Expr::Map — infers list type from mapped-expression type
// ---------------------------------------------------------------------------

#[test]
fn infer_map_identity_i32() {
    // map([1, 2, 3], item => item)
    // base type: List<I32>; map body returns arg(0) which is I32
    // result: List<I32>
    let base = Expr::list([Value::I32(1), Value::I32(2)]);
    let map_body = Expr::arg(ExprArg::new(0));
    let expr = Expr::map(base, map_body);
    assert_eq!(infer(&expr), Type::list(Type::I32));
}

#[test]
fn infer_map_arg_type_from_outer_args() {
    // map(arg(0), item => item)  where arg(0) is List<String>
    // The outer arg resolves to List<String>, so the item type in the map
    // scope is String, and the map body (arg(0) at nesting=0) is String.
    let base = Expr::arg(ExprArg { position: 0, nesting: 0 });
    let map_body = Expr::arg(ExprArg { position: 0, nesting: 0 });
    let expr = Expr::map(base, map_body);
    assert_eq!(
        infer_with_args(&expr, &[Type::list(Type::String)]),
        Type::list(Type::String)
    );
}

#[test]
fn infer_map_bool_to_bool() {
    // map([true, false], item => item)  →  List<Bool>
    let base = Expr::list([Value::Bool(true), Value::Bool(false)]);
    let map_body = Expr::arg(ExprArg::new(0));
    let expr = Expr::map(base, map_body);
    assert_eq!(infer(&expr), Type::list(Type::Bool));
}

// ---------------------------------------------------------------------------
// Expr::Project — unwraps field type from record
// ---------------------------------------------------------------------------

#[test]
fn infer_project_first_field() {
    // project((I32, String), 0) → I32
    let base = Expr::record([Expr::from(Value::I32(1)), Expr::from("x")]);
    let expr = Expr::project(base, 0usize);
    assert_eq!(infer(&expr), Type::I32);
}

#[test]
fn infer_project_second_field() {
    // project((I32, String), 1) → String
    let base = Expr::record([Expr::from(Value::I32(1)), Expr::from("x")]);
    let expr = Expr::project(base, 1usize);
    assert_eq!(infer(&expr), Type::String);
}

#[test]
fn infer_project_nested_record() {
    // project((I32, (Bool, String)), 1) → (Bool, String)
    let inner = Expr::record([Expr::from(Value::Bool(true)), Expr::from("y")]);
    let outer = Expr::record([Expr::from(Value::I32(0)), inner]);
    let expr = Expr::project(outer, 1usize);
    assert_eq!(infer(&expr), Type::Record(vec![Type::Bool, Type::String]));
}
