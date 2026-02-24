use toasty_core::stmt::{
    BinaryOp, Expr, ExprAnd, ExprBinaryOp, ExprIsNull, ExprList, ExprNot, ExprRecord, Value,
};

// ---------------------------------------------------------------------------
// From<ExprList> for Expr
// ---------------------------------------------------------------------------

#[test]
fn from_expr_list_for_expr() {
    let list = ExprList {
        items: vec![Expr::Value(Value::Bool(true)), Expr::Value(Value::I64(1))],
    };
    assert_eq!(
        Expr::from(list),
        Expr::List(ExprList {
            items: vec![Expr::Value(Value::Bool(true)), Expr::Value(Value::I64(1))],
        })
    );
}

#[test]
fn from_expr_list_empty_for_expr() {
    assert_eq!(
        Expr::from(ExprList { items: vec![] }),
        Expr::List(ExprList { items: vec![] })
    );
}

// ---------------------------------------------------------------------------
// From<Vec<Expr>> for Expr  (creates ExprList via list_from_vec)
// ---------------------------------------------------------------------------

#[test]
fn from_vec_expr_for_expr() {
    let items = vec![Expr::Value(Value::I64(1)), Expr::Value(Value::I64(2))];
    assert_eq!(
        Expr::from(items),
        Expr::List(ExprList {
            items: vec![Expr::Value(Value::I64(1)), Expr::Value(Value::I64(2))],
        })
    );
}

#[test]
fn from_vec_expr_empty_for_expr() {
    assert_eq!(
        Expr::from(Vec::<Expr>::new()),
        Expr::List(ExprList { items: vec![] })
    );
}

// ---------------------------------------------------------------------------
// From<ExprRecord> for Expr
// ---------------------------------------------------------------------------

#[test]
fn from_expr_record_for_expr() {
    let record = ExprRecord {
        fields: vec![
            Expr::Value(Value::Bool(true)),
            Expr::Value(Value::from("hi")),
        ],
    };
    assert_eq!(
        Expr::from(record),
        Expr::Record(ExprRecord {
            fields: vec![
                Expr::Value(Value::Bool(true)),
                Expr::Value(Value::from("hi"))
            ],
        })
    );
}

// ---------------------------------------------------------------------------
// From<(E1, E2)> for Expr  (2-tuple via expr.rs → ExprRecord)
// ---------------------------------------------------------------------------

#[test]
fn from_tuple2_for_expr() {
    // The From<(E1,E2)> for Expr impl calls Self::Record(value.into()),
    // which calls From<(E1,E2)> for ExprRecord.
    let expr = Expr::from((true, 42i64));
    assert_eq!(
        expr,
        Expr::Record(ExprRecord {
            fields: vec![Expr::Value(Value::Bool(true)), Expr::Value(Value::I64(42)),],
        })
    );
}

// ---------------------------------------------------------------------------
// From<(T0, T1, T2)> for ExprRecord  (3-tuple → ExprRecord → Expr)
// ---------------------------------------------------------------------------

#[test]
fn from_tuple3_for_expr_record() {
    let record = ExprRecord::from((true, 1i64, "hi"));
    assert_eq!(
        record,
        ExprRecord {
            fields: vec![
                Expr::Value(Value::Bool(true)),
                Expr::Value(Value::I64(1)),
                Expr::Value(Value::from("hi")),
            ],
        }
    );
}

// ---------------------------------------------------------------------------
// From<ExprAnd> for Expr
// ---------------------------------------------------------------------------

#[test]
fn from_expr_and_for_expr() {
    let and = ExprAnd {
        operands: vec![
            Expr::Value(Value::Bool(true)),
            Expr::Value(Value::Bool(false)),
        ],
    };
    assert_eq!(
        Expr::from(and),
        Expr::And(ExprAnd {
            operands: vec![
                Expr::Value(Value::Bool(true)),
                Expr::Value(Value::Bool(false))
            ],
        })
    );
}

// ---------------------------------------------------------------------------
// From<ExprNot> for Expr
// ---------------------------------------------------------------------------

#[test]
fn from_expr_not_for_expr() {
    let not = ExprNot {
        expr: Box::new(Expr::Value(Value::Bool(true))),
    };
    assert_eq!(
        Expr::from(not),
        Expr::Not(ExprNot {
            expr: Box::new(Expr::Value(Value::Bool(true))),
        })
    );
}

// ---------------------------------------------------------------------------
// From<ExprIsNull> for Expr
// ---------------------------------------------------------------------------

#[test]
fn from_expr_is_null_for_expr() {
    let is_null = ExprIsNull {
        expr: Box::new(Expr::Value(Value::Null)),
    };
    assert_eq!(
        Expr::from(is_null),
        Expr::IsNull(ExprIsNull {
            expr: Box::new(Expr::Value(Value::Null)),
        })
    );
}

// ---------------------------------------------------------------------------
// From<ExprBinaryOp> for Expr
// ---------------------------------------------------------------------------

#[test]
fn from_expr_binary_op_for_expr() {
    let op = ExprBinaryOp {
        lhs: Box::new(Expr::Value(Value::I64(1))),
        op: BinaryOp::Eq,
        rhs: Box::new(Expr::Value(Value::I64(1))),
    };
    assert_eq!(
        Expr::from(op),
        Expr::BinaryOp(ExprBinaryOp {
            lhs: Box::new(Expr::Value(Value::I64(1))),
            op: BinaryOp::Eq,
            rhs: Box::new(Expr::Value(Value::I64(1))),
        })
    );
}
