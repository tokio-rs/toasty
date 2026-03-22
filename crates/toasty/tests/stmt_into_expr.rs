use toasty::stmt::{Expr, IntoExpr, List};
use toasty_core::stmt as core_stmt;

// ---------------------------------------------------------------------------
// Helper: unwrap the core expression from a typed Expr<T>
// ---------------------------------------------------------------------------

fn untyped<T>(expr: Expr<T>) -> core_stmt::Expr {
    core_stmt::Expr::from(expr)
}

// ---------------------------------------------------------------------------
// Primitive scalar impls
// ---------------------------------------------------------------------------

#[test]
fn into_expr_bool() {
    let expr: Expr<bool> = true.into_expr();
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::Bool(true))
    );
}

#[test]
fn into_expr_i64() {
    let expr: Expr<i64> = 42i64.into_expr();
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::I64(42))
    );
}

#[test]
fn into_expr_string() {
    let expr: Expr<String> = String::from("hello").into_expr();
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::String("hello".into()))
    );
}

#[test]
fn into_expr_str_ref() {
    let expr: Expr<String> = "world".into_expr();
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::String("world".into()))
    );
}

#[test]
fn into_expr_uuid() {
    let id = uuid::Uuid::nil();
    let expr: Expr<uuid::Uuid> = id.into_expr();
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::Uuid(uuid::Uuid::nil()))
    );
}

// ---------------------------------------------------------------------------
// by_ref for scalars
// ---------------------------------------------------------------------------

#[test]
fn by_ref_i64() {
    let val = 7i64;
    let expr: Expr<i64> = val.by_ref();
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::I64(7))
    );
}

#[test]
fn by_ref_string() {
    let val = String::from("ref");
    let expr: Expr<String> = val.by_ref();
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::String("ref".into()))
    );
}

// ---------------------------------------------------------------------------
// Expr<T> identity impl
// ---------------------------------------------------------------------------

#[test]
fn into_expr_identity() {
    let original: Expr<i64> = 99i64.into_expr();
    let cloned_untyped = untyped(original.clone());
    let roundtripped: Expr<i64> = original.into_expr();
    assert_eq!(untyped(roundtripped), cloned_untyped);
}

#[test]
fn by_ref_identity() {
    let original: Expr<i64> = 99i64.into_expr();
    let cloned_untyped = untyped(original.clone());
    let by_ref_expr: Expr<i64> = original.by_ref();
    assert_eq!(untyped(by_ref_expr), cloned_untyped);
}

// ---------------------------------------------------------------------------
// &T delegates to T::by_ref
// ---------------------------------------------------------------------------

#[test]
fn into_expr_ref_delegates() {
    let val = 5i64;
    let from_ref: Expr<i64> = (&val).into_expr();
    let from_by_ref: Expr<i64> = val.by_ref();
    assert_eq!(untyped(from_ref), untyped(from_by_ref));
}

// ---------------------------------------------------------------------------
// Option<T>
// ---------------------------------------------------------------------------

#[test]
fn into_expr_some() {
    let expr: Expr<Option<i64>> = Some(10i64).into_expr();
    // Some wraps via cast, so the inner value should be an i64
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::I64(10))
    );
}

#[test]
fn into_expr_none() {
    let expr: Expr<Option<i64>> = None::<i64>.into_expr();
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::Null)
    );
}

#[test]
fn by_ref_some() {
    let val: Option<i64> = Some(10);
    let expr: Expr<Option<i64>> = val.by_ref();
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::I64(10))
    );
}

#[test]
fn by_ref_none() {
    let val: Option<i64> = None;
    let expr: Expr<Option<i64>> = val.by_ref();
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::Null)
    );
}

// ---------------------------------------------------------------------------
// T -> Option<T>
// ---------------------------------------------------------------------------

#[test]
fn into_expr_value_as_option() {
    let expr: Expr<Option<i64>> = 42i64.into_expr();
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::I64(42))
    );
}

// ---------------------------------------------------------------------------
// &Option<T> -> T
// ---------------------------------------------------------------------------

#[test]
fn into_expr_ref_option_some() {
    let val: Option<i64> = Some(3);
    let expr: Expr<i64> = (&val).into_expr();
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::I64(3))
    );
}

#[test]
fn into_expr_ref_option_none() {
    let val: Option<i64> = None;
    let expr: Expr<i64> = (&val).into_expr();
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::Null)
    );
}

// ---------------------------------------------------------------------------
// Arrays [U; N] -> List<T>
// ---------------------------------------------------------------------------

#[test]
fn into_expr_array_to_list() {
    let arr = [1i64, 2, 3];
    let expr: Expr<List<i64>> = arr.into_expr();
    let expected = core_stmt::Expr::list([
        core_stmt::Expr::Value(core_stmt::Value::I64(1)),
        core_stmt::Expr::Value(core_stmt::Value::I64(2)),
        core_stmt::Expr::Value(core_stmt::Value::I64(3)),
    ]);
    assert_eq!(untyped(expr), expected);
}

#[test]
fn into_expr_empty_array_to_list() {
    let arr: [i64; 0] = [];
    let expr: Expr<List<i64>> = arr.into_expr();
    let expected = core_stmt::Expr::list(std::iter::empty::<core_stmt::Expr>());
    assert_eq!(untyped(expr), expected);
}

#[test]
fn by_ref_array_to_list() {
    let arr = [1i64, 2];
    let expr: Expr<List<i64>> = arr.by_ref();
    let expected = core_stmt::Expr::list([
        core_stmt::Expr::Value(core_stmt::Value::I64(1)),
        core_stmt::Expr::Value(core_stmt::Value::I64(2)),
    ]);
    assert_eq!(untyped(expr), expected);
}

// ---------------------------------------------------------------------------
// &[U; N] -> List<T>
// ---------------------------------------------------------------------------

#[test]
fn into_expr_ref_array_to_list() {
    let arr = [10i64, 20];
    let expr: Expr<List<i64>> = (&arr).into_expr();
    let expected = core_stmt::Expr::list([
        core_stmt::Expr::Value(core_stmt::Value::I64(10)),
        core_stmt::Expr::Value(core_stmt::Value::I64(20)),
    ]);
    assert_eq!(untyped(expr), expected);
}

// ---------------------------------------------------------------------------
// Vec<U> -> List<T>
// ---------------------------------------------------------------------------

#[test]
fn into_expr_vec_to_list() {
    let v = vec![4i64, 5, 6];
    let expr: Expr<List<i64>> = v.into_expr();
    let expected = core_stmt::Expr::list([
        core_stmt::Expr::Value(core_stmt::Value::I64(4)),
        core_stmt::Expr::Value(core_stmt::Value::I64(5)),
        core_stmt::Expr::Value(core_stmt::Value::I64(6)),
    ]);
    assert_eq!(untyped(expr), expected);
}

#[test]
fn into_expr_empty_vec_to_list() {
    let v: Vec<i64> = vec![];
    let expr: Expr<List<i64>> = v.into_expr();
    let expected = core_stmt::Expr::list(std::iter::empty::<core_stmt::Expr>());
    assert_eq!(untyped(expr), expected);
}

#[test]
fn by_ref_vec_to_list() {
    let v = vec![7i64, 8];
    let expr: Expr<List<i64>> = v.by_ref();
    let expected = core_stmt::Expr::list([
        core_stmt::Expr::Value(core_stmt::Value::I64(7)),
        core_stmt::Expr::Value(core_stmt::Value::I64(8)),
    ]);
    assert_eq!(untyped(expr), expected);
}

// ---------------------------------------------------------------------------
// &[E] -> List<T>
// ---------------------------------------------------------------------------

#[test]
fn into_expr_slice_to_list() {
    let v = vec![100i64, 200];
    let slice: &[i64] = &v;
    let expr: Expr<List<i64>> = slice.into_expr();
    let expected = core_stmt::Expr::list([
        core_stmt::Expr::Value(core_stmt::Value::I64(100)),
        core_stmt::Expr::Value(core_stmt::Value::I64(200)),
    ]);
    assert_eq!(untyped(expr), expected);
}

// ---------------------------------------------------------------------------
// &T -> List<T> (single-element via cast)
// ---------------------------------------------------------------------------

#[test]
fn into_expr_ref_to_list() {
    let val = 42i64;
    let expr: Expr<List<i64>> = (&val).into_expr();
    // &T -> Expr<List<T>> uses by_ref().cast()
    assert_eq!(
        untyped(expr),
        core_stmt::Expr::Value(core_stmt::Value::I64(42))
    );
}

// ---------------------------------------------------------------------------
// Tuple impls
// ---------------------------------------------------------------------------

#[test]
fn into_expr_pair() {
    let pair: (i64, bool) = (1, true);
    let expr: Expr<(i64, bool)> = pair.into_expr();
    let core_expr = untyped(expr);
    let expected = core_stmt::Expr::Record(core_stmt::ExprRecord::from_vec(vec![
        core_stmt::Expr::Value(core_stmt::Value::I64(1)),
        core_stmt::Expr::Value(core_stmt::Value::Bool(true)),
    ]));
    assert_eq!(core_expr, expected);
}

#[test]
fn into_expr_triple() {
    let triple = (1i64, "hi", true);
    let expr: Expr<(i64, String, bool)> = triple.into_expr();
    let core_expr = untyped(expr);
    let expected = core_stmt::Expr::Record(core_stmt::ExprRecord::from_vec(vec![
        core_stmt::Expr::Value(core_stmt::Value::I64(1)),
        core_stmt::Expr::Value(core_stmt::Value::String("hi".into())),
        core_stmt::Expr::Value(core_stmt::Value::Bool(true)),
    ]));
    assert_eq!(core_expr, expected);
}

#[test]
fn by_ref_pair() {
    let pair = (10i64, false);
    let expr: Expr<(i64, bool)> = pair.by_ref();
    let core_expr = untyped(expr);
    let expected = core_stmt::Expr::Record(core_stmt::ExprRecord::from_vec(vec![
        core_stmt::Expr::Value(core_stmt::Value::I64(10)),
        core_stmt::Expr::Value(core_stmt::Value::Bool(false)),
    ]));
    assert_eq!(core_expr, expected);
}

// ---------------------------------------------------------------------------
// Batch<T> -> IntoExpr<T>
// ---------------------------------------------------------------------------

#[test]
fn batch_into_expr_wraps_statement() {
    let query = core_stmt::Query::new_single(core_stmt::Values::from(core_stmt::Expr::Value(
        core_stmt::Value::I64(1),
    )));
    let stmt = toasty::Statement::<i64>::from_untyped_stmt(query.into());
    let batch = toasty::stmt::Batch::from(stmt);
    let expr: Expr<i64> = batch.into_expr();
    let core_expr = untyped(expr);

    // Should be an Expr::Stmt wrapping the original statement
    assert!(matches!(core_expr, core_stmt::Expr::Stmt(_)));
}

#[test]
fn batch_by_ref_wraps_statement() {
    let query = core_stmt::Query::new_single(core_stmt::Values::from(core_stmt::Expr::Value(
        core_stmt::Value::I64(1),
    )));
    let stmt = toasty::Statement::<i64>::from_untyped_stmt(query.into());
    let batch = toasty::stmt::Batch::from(stmt);
    let expr: Expr<i64> = batch.by_ref();
    let core_expr = untyped(expr);

    assert!(matches!(core_expr, core_stmt::Expr::Stmt(_)));
}

#[test]
fn batch_into_expr_and_by_ref_agree() {
    let query = core_stmt::Query::new_single(core_stmt::Values::from(core_stmt::Expr::Value(
        core_stmt::Value::Bool(true),
    )));
    let stmt = toasty::Statement::<bool>::from_untyped_stmt(query.into());
    let batch = toasty::stmt::Batch::from(stmt);

    let by_ref_expr: Expr<bool> = batch.by_ref();
    let owned_expr: Expr<bool> = batch.into_expr();

    assert_eq!(untyped(by_ref_expr), untyped(owned_expr));
}
