use assert_struct::assert_struct;
use tests::{models, prelude::*, tests, DbTest};
use toasty::stmt::Id;
use toasty_core::{
    driver::Operation,
    stmt::{
        BinaryOp, Expr, ExprColumn, ExprSet, Limit, OrderBy, OrderByExpr, Source, Statement, Value,
    },
};

#[derive(toasty::Model)]
struct Foo {
    #[key]
    #[auto]
    id: Id<Self>,

    #[index]
    order: i64,
}

async fn sort_asc(test: &mut DbTest) {
    if !test.capability().sql {
        return;
    }

    let db = test.setup_db(models!(Foo)).await;

    for i in 0..100 {
        Foo::create().order(i).exec(&db).await.unwrap();
    }

    let foos_asc: Vec<_> = Foo::all()
        .order_by(Foo::FIELDS.order().asc())
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(foos_asc.len(), 100);

    for i in 0..99 {
        assert!(foos_asc[i].order < foos_asc[i + 1].order);
    }

    let foos_desc: Vec<_> = Foo::all()
        .order_by(Foo::FIELDS.order().desc())
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(foos_desc.len(), 100);

    for i in 0..99 {
        assert!(foos_desc[i].order > foos_desc[i + 1].order);
    }
}

async fn paginate(test: &mut DbTest) {
    if !test.is_sql() {
        return;
    }

    let db = test.setup_db(models!(Foo)).await;
    let foo_table_id = table(&db, "foos");

    for i in 0..100 {
        Foo::create().order(i).exec(&db).await.unwrap();
    }

    // Clear setup operations
    test.log().clear();

    // ========== FIRST PAGE ==========
    let page = Foo::all()
        .order_by(Foo::FIELDS.order().desc())
        .paginate(10)
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(page.items.len(), 10);
    // There are 100 items total, we're on the first page of 10, so there should be more pages
    assert!(page.has_next(), "First page should have next");
    assert!(!page.has_prev(), "First page should not have prev");

    for (i, order) in (99..90).enumerate() {
        assert_eq!(page.items[i].order, order);
    }

    // Check the first page query operation
    let (op, _) = test.log().pop();

    if test.is_sql() {
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Query(_ {
                body: ExprSet::Select(_ {
                    source: =~ table(&db, "foos"),
                    ..
                }),
                order_by: Some(OrderBy {
                    exprs: [OrderByExpr {
                        expr: =~ column(&db, "foos", "order"),
                        order: Some(_),
                        ..
                    }],
                    ..
                }),
                limit: Some(Limit::Offset {
                    limit: =~ 11,
                    offset: None,
                }),
                ..
            }),
            ..
        }));
    } else {
        // For NoSQL databases, expect FindPkByIndex operation
        assert_struct!(op, Operation::FindPkByIndex(_ {
            table: == table(&db, "foos"),
            index: == index(&db, "foos", "order"),
            filter: _,
            ..
        }));
    }

    // ========== SECOND PAGE ==========
    let page = Foo::all()
        .order_by(Foo::FIELDS.order().desc())
        .paginate(10)
        .after(90)
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(page.items.len(), 10);
    assert!(page.has_next(), "Second page should have next");
    // Note: prev cursor is not implemented yet, so this will be false
    assert!(!page.has_prev(), "Prev cursor not implemented yet");

    for (i, order) in (89..80).enumerate() {
        assert_eq!(page.items[i].order, order);
    }

    // Check the second page query operation
    let (op, _) = test.log().pop();

    if test.is_sql() {
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Query(_ {
                body: ExprSet::Select(_ {
                    source: Source::Table([
                        _ { table: foo_table_id, .. },
                    ]),
                    filter: Expr::BinaryOp(_ {
                        *lhs: =~ column(&db, "foos", "order"),
                        op: BinaryOp::Lt,
                        *rhs: =~ 90,
                        ..
                    }),
                    ..
                }),
                order_by: Some(OrderBy {
                    exprs: [OrderByExpr {
                        expr: Expr::Column(ExprColumn::Column(== column(&db, "foos", "order"))),
                        order: Some(_),
                        ..
                    }, ..],
                    ..
                }),
                limit: Some(Limit::Offset {
                    limit: Expr::Value(Value::I64(11)),
                    offset: None,
                }),
                ..
            }),
            ..
        }));
    } else {
        // For NoSQL databases, expect FindPkByIndex operation with filter
        assert_struct!(op, Operation::FindPkByIndex(_ {
            table: foo_table_id,
            filter: Expr::BinaryOp(_ {
                *lhs: Expr::Column(ExprColumn::Column(== column(&db, "foos", "order"))),
                op: BinaryOp::Lt,
                *rhs: Expr::Value(Value::I64(90)),
                ..
            }),
            ..
        }));
    }

    // ========== LAST PAGE ==========
    let last_page = Foo::all()
        .order_by(Foo::FIELDS.order().desc())
        .paginate(10)
        .after(10)
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(last_page.items.len(), 10);
    assert!(!last_page.has_next(), "Last page should not have next");

    for (i, order) in (9..0).enumerate() {
        assert_eq!(last_page.items[i].order, order);
    }

    // Check the last page query operation
    let (op, _) = test.log().pop();

    if test.is_sql() {
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Query(_ {
                body: ExprSet::Select(_ {
                    source: Source::Table([
                        _ { table: foo_table_id, .. },
                    ]),
                    filter: Expr::BinaryOp(_ {
                        *lhs: Expr::Column(ExprColumn::Column(== column(&db, "foos", "order"))),
                        op: BinaryOp::Lt,
                        *rhs: Expr::Value(Value::I64(10)),
                        ..
                    }),
                    ..
                }),
                order_by: Some(OrderBy {
                    exprs: [OrderByExpr {
                        expr: =~ column(&db, "foos", "order"),
                        order: Some(_),
                        ..
                    }, ..],
                    ..
                }),
                limit: Some(Limit::Offset {
                    limit: Expr::Value(Value::I64(11)),
                    offset: None,
                }),
                ..
            }),
            ..
        }));
    } else {
        // For NoSQL databases, expect FindPkByIndex operation with filter
        assert_struct!(op, Operation::FindPkByIndex(_ {
            table: foo_table_id,
            filter: Expr::BinaryOp(_ {
                *lhs: Expr::Column(ExprColumn::Column(== column(&db, "foos", "order"))),
                op: BinaryOp::Lt,
                *rhs: Expr::Value(Value::I64(10)),
                ..
            }),
            ..
        }));
    }

    // Verify log is empty
    assert!(
        test.log().is_empty(),
        "Log should be empty after all assertions"
    );
}

tests!(sort_asc, paginate,);
