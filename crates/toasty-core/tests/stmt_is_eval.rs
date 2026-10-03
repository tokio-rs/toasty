use toasty_core::{
    schema::app::{ModelId, VariantId},
    stmt::{
        Expr, ExprReference, FuncJsonExtract, MatchArm, Query, Type, Value, ValueObject, Values,
    },
};

#[test]
fn operations_requiring_lowering_or_database_are_not_eval() {
    let variant = VariantId {
        model: ModelId(0),
        index: 0,
    };
    for expr in [
        Expr::between(2, 1, 3),
        Expr::is_variant(0, variant),
        Expr::variant(0, variant),
        Expr::array_is_superset(Expr::list([1, 2]), Expr::list([1])),
        Expr::array_intersects(Expr::list([1, 2]), Expr::list([1])),
        Expr::array_length(Expr::list([1, 2])),
    ] {
        assert!(!expr.is_eval(), "{expr:?}");
    }
}

#[test]
fn match_checks_fallback_evaluability() {
    let expr = Expr::match_expr(
        1,
        vec![MatchArm {
            pattern: 0.into(),
            expr: true.into(),
        }],
        Expr::count_star(),
    );
    assert!(!expr.is_eval());

    let expr = Expr::match_expr(1, vec![], Expr::error("no matching arm"));
    assert!(expr.is_eval());
    assert!(expr.eval_const().is_err());
}

#[test]
fn exists_values_are_eval_when_rows_are_eval() {
    for (rows, expected) in [(vec![], false), (vec![Expr::from(1)], true)] {
        let expr = Expr::exists(Query::values(Values::new(rows)));
        assert!(expr.is_eval());
        assert_eq!(expr.eval_const().unwrap(), Value::Bool(expected));
    }

    let reference = Expr::Reference(ExprReference::Model { nesting: 0 });
    let expr = Expr::exists(Query::values(Values::new(vec![reference])));
    assert!(!expr.is_eval());
    assert!(!Expr::exists(Query::new_select(ModelId(0), true)).is_eval());
}

#[test]
fn json_extract_is_eval_when_base_is_eval() {
    let object = Value::Object(ValueObject::from_vec(vec![("answer".into(), 42.into())]));
    let extract = |base: Expr| {
        Expr::from(FuncJsonExtract {
            base: Box::new(base),
            path: vec!["answer".into()],
            ty: Type::I64,
        })
    };
    let expr = extract(object.into());
    assert!(expr.is_eval());
    assert_eq!(expr.eval_const().unwrap(), Value::I64(42));
    assert!(extract(Expr::arg(0)).is_eval());
    assert!(!extract(Expr::Reference(ExprReference::Model { nesting: 0 })).is_eval());
}
