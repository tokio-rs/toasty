use toasty_core::stmt::{BinaryOp, Expr, ExprArg, ExprReference, Projection, Type};

fn outer_arg() -> Expr {
    // nesting=1: escapes the innermost Map scope, references an outer argument
    Expr::arg(ExprArg {
        position: 0,
        nesting: 1,
    })
}

// Helpers
fn val() -> Expr {
    Expr::from(42i64)
}
fn arg() -> Expr {
    Expr::arg(ExprArg::new(0))
}
fn reference() -> Expr {
    Expr::Reference(ExprReference::Model { nesting: 0 })
}

// ---------------------------------------------------------------------------
// Leaf: Expr::Value — stable, const, eval
// ---------------------------------------------------------------------------

#[test]
fn value_is_stable() {
    assert!(val().is_stable());
}

#[test]
fn value_is_const() {
    assert!(val().is_const());
}

#[test]
fn value_is_eval() {
    assert!(val().is_eval());
}

// ---------------------------------------------------------------------------
// Leaf: Expr::Default — not stable, not const, not eval
// ---------------------------------------------------------------------------

#[test]
fn default_not_stable() {
    assert!(!Expr::Default.is_stable());
}

#[test]
fn default_not_const() {
    assert!(!Expr::Default.is_const());
}

#[test]
fn default_not_eval() {
    assert!(!Expr::Default.is_eval());
}

// ---------------------------------------------------------------------------
// Leaf: Expr::Error — stable, const, eval
// ---------------------------------------------------------------------------

#[test]
fn error_is_stable() {
    assert!(Expr::error("boom").is_stable());
}

#[test]
fn error_is_const() {
    assert!(Expr::error("boom").is_const());
}

#[test]
fn error_is_eval() {
    assert!(Expr::error("boom").is_eval());
}

// ---------------------------------------------------------------------------
// Leaf: Expr::Reference — stable, not const, not eval
// ---------------------------------------------------------------------------

#[test]
fn reference_is_stable() {
    assert!(reference().is_stable());
}

#[test]
fn reference_not_const() {
    assert!(!reference().is_const());
}

#[test]
fn reference_not_eval() {
    assert!(!reference().is_eval());
}

// ---------------------------------------------------------------------------
// Leaf: Expr::Arg — stable, not const, eval
// ---------------------------------------------------------------------------

#[test]
fn arg_is_stable() {
    assert!(arg().is_stable());
}

#[test]
fn arg_not_const() {
    assert!(!arg().is_const());
}

#[test]
fn arg_is_eval() {
    assert!(arg().is_eval());
}

// ---------------------------------------------------------------------------
// Composites with all-const children — stable, const, eval
// ---------------------------------------------------------------------------

#[test]
fn and_const_const_is_stable_const_eval() {
    let e = Expr::and(val(), val());
    assert!(e.is_stable());
    assert!(e.is_const());
    assert!(e.is_eval());
}

#[test]
fn or_const_const_is_stable_const_eval() {
    let e = Expr::or(val(), val());
    assert!(e.is_stable());
    assert!(e.is_const());
    assert!(e.is_eval());
}

#[test]
fn not_const_is_stable_const_eval() {
    let e = Expr::not(val());
    assert!(e.is_stable());
    assert!(e.is_const());
    assert!(e.is_eval());
}

#[test]
fn is_null_const_is_stable_const_eval() {
    let e = Expr::is_null(val());
    assert!(e.is_stable());
    assert!(e.is_const());
    assert!(e.is_eval());
}

#[test]
fn cast_const_is_stable_const_eval() {
    let e = Expr::cast(val(), Type::I64);
    assert!(e.is_stable());
    assert!(e.is_const());
    assert!(e.is_eval());
}

#[test]
fn binary_op_const_const_is_stable_const_eval() {
    let e = Expr::binary_op(val(), BinaryOp::Eq, val());
    assert!(e.is_stable());
    assert!(e.is_const());
    assert!(e.is_eval());
}

#[test]
fn in_list_const_const_is_stable_const_eval() {
    let e = Expr::in_list(val(), Expr::list([val(), val()]));
    assert!(e.is_stable());
    assert!(e.is_const());
    assert!(e.is_eval());
}

#[test]
fn project_const_is_stable_const_eval() {
    let e = Expr::project(Expr::record([val(), val()]), Projection::single(0));
    assert!(e.is_stable());
    assert!(e.is_const());
    assert!(e.is_eval());
}

#[test]
fn record_all_const_is_stable_const_eval() {
    let e = Expr::record([val(), val(), val()]);
    assert!(e.is_stable());
    assert!(e.is_const());
    assert!(e.is_eval());
}

#[test]
fn list_all_const_is_stable_const_eval() {
    let e = Expr::list([val(), val()]);
    assert!(e.is_stable());
    assert!(e.is_const());
    assert!(e.is_eval());
}

// ---------------------------------------------------------------------------
// Composites with a Reference child — stable, not const, not eval
// ---------------------------------------------------------------------------

#[test]
fn and_with_reference_not_const_not_eval() {
    let e = Expr::and(val(), reference());
    assert!(e.is_stable());
    assert!(!e.is_const());
    assert!(!e.is_eval());
}

#[test]
fn record_with_reference_not_const_not_eval() {
    let e = Expr::record([val(), reference()]);
    assert!(e.is_stable());
    assert!(!e.is_const());
    assert!(!e.is_eval());
}

#[test]
fn project_reference_base_not_const_not_eval() {
    let e = Expr::project(reference(), Projection::single(0));
    assert!(e.is_stable());
    assert!(!e.is_const());
    assert!(!e.is_eval());
}

// ---------------------------------------------------------------------------
// Composites with an Arg child — stable, not const, eval
// ---------------------------------------------------------------------------

#[test]
fn and_with_arg_not_const_is_eval() {
    let e = Expr::and(val(), arg());
    assert!(e.is_stable());
    assert!(!e.is_const());
    assert!(e.is_eval());
}

#[test]
fn record_with_arg_not_const_is_eval() {
    let e = Expr::record([val(), arg()]);
    assert!(e.is_stable());
    assert!(!e.is_const());
    assert!(e.is_eval());
}

// ---------------------------------------------------------------------------
// Composites with a Default child — not stable, not const, not eval
// ---------------------------------------------------------------------------

#[test]
fn and_with_default_not_stable() {
    let e = Expr::and(val(), Expr::Default);
    assert!(!e.is_stable());
    assert!(!e.is_const());
    assert!(!e.is_eval());
}

#[test]
fn record_with_default_not_stable() {
    let e = Expr::record([val(), Expr::Default]);
    assert!(!e.is_stable());
    assert!(!e.is_const());
    assert!(!e.is_eval());
}

// ---------------------------------------------------------------------------
// Expr::Map — all three properties
// ---------------------------------------------------------------------------

#[test]
fn map_const_base_local_arg_body_is_stable_const_eval() {
    // arg(nesting=0) inside the map body is a local binding (the mapped element)
    let e = Expr::map(Expr::list([val(), val()]), arg());
    assert!(e.is_stable());
    assert!(e.is_const());
    assert!(e.is_eval());
}

#[test]
fn map_const_base_outer_arg_body_not_const() {
    // arg(nesting=1) escapes the map scope — references an outer arg, not const
    let e = Expr::map(Expr::list([val()]), outer_arg());
    assert!(e.is_stable());
    assert!(!e.is_const());
    assert!(e.is_eval()); // is_eval doesn't distinguish nesting
}

#[test]
fn map_const_base_reference_body_not_const_not_eval() {
    // map body references external data → not const, not eval
    let e = Expr::map(Expr::list([val()]), reference());
    assert!(e.is_stable());
    assert!(!e.is_const());
    assert!(!e.is_eval());
}

#[test]
fn map_default_base_not_stable_not_const_not_eval() {
    let e = Expr::map(Expr::Default, arg());
    assert!(!e.is_stable());
    assert!(!e.is_const());
    assert!(!e.is_eval());
}

#[test]
fn map_reference_base_is_stable_not_const_not_eval() {
    let e = Expr::map(reference(), arg());
    assert!(e.is_stable());
    assert!(!e.is_const());
    assert!(!e.is_eval());
}

#[test]
fn nested_map_inner_local_arg_is_const() {
    // map(list, map(list, arg(nesting=0)))
    // arg(nesting=0) is local to the inner map — const
    let inner = Expr::map(Expr::list([val()]), arg());
    let outer = Expr::map(Expr::list([val()]), inner);
    assert!(outer.is_const());
}

#[test]
fn nested_map_outer_element_arg_is_const() {
    // map(list, x => map(list, y => x))  i.e. arg(nesting=1) in the inner body
    // arg(nesting=1) refers to the outer map's current element — still a local binding
    let inner = Expr::map(Expr::list([val()]), outer_arg());
    let outer = Expr::map(Expr::list([val()]), inner);
    assert!(outer.is_const());
}

#[test]
fn nested_map_escape_both_scopes_not_const() {
    // map(list, map(list, arg(nesting=2)))
    // arg(nesting=2) escapes both map scopes — references truly external data, not const
    let escape = Expr::arg(ExprArg {
        position: 0,
        nesting: 2,
    });
    let inner = Expr::map(Expr::list([val()]), escape);
    let outer = Expr::map(Expr::list([val()]), inner);
    assert!(!outer.is_const());
}
