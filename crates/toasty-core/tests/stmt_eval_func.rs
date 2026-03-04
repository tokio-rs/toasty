use toasty_core::stmt::Expr;

#[test]
fn count_star_is_not_eval() {
    assert!(!Expr::count_star().is_eval());
}

#[test]
fn count_star_eval_is_error() {
    assert!(Expr::count_star().eval_const().is_err());
}

#[test]
fn last_insert_id_is_not_eval() {
    assert!(!Expr::last_insert_id().is_eval());
}

#[test]
fn last_insert_id_eval_is_error() {
    assert!(Expr::last_insert_id().eval_const().is_err());
}
