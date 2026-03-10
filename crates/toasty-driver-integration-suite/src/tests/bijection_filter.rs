use crate::helpers::column;
use crate::prelude::*;

use toasty_core::{
    driver::Operation,
    stmt::{self, ExprSet, Statement},
};

/// Verify that filtering a Timestamp field stored as TEXT correctly encodes the
/// filter value via the Cast bijection. The WHERE clause should compare the raw
/// TEXT column against the string-encoded timestamp — not wrap the column in a
/// Cast and compare against a native Timestamp value.
#[driver_test(id(ID), requires(sql))]
pub async fn filter_timestamp_stored_as_text(test: &mut Test) -> Result<(), BoxError> {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,
        #[index]
        #[column(type = text)]
        val: Timestamp,
    }

    let mut db = test.setup_db(models!(Foo)).await;

    let ts = Timestamp::from_second(946684800)?; // 2000-01-01T00:00:00Z
    let created = Foo::create().val(ts).exec(&mut db).await?;

    test.log().clear();

    // Filter by the Jiff Timestamp value
    let results = Foo::filter_by_val(ts).collect::<Vec<_>>(&mut db).await?;

    // Behavioral: correct record returned
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].val, ts);
    assert_eq!(results[0].id, created.id);

    // Structural: the filter sent to the driver uses the text-encoded value.
    // The bijection (Cast { from: Timestamp, to: String }) should encode the
    // Timestamp to its string representation and compare directly against the
    // TEXT column.
    let (op, _) = test.log().pop();
    let ts_text = ts.to_string();
    let val_column = column(&db, "foos", "val");

    assert_struct!(&op, Operation::QuerySql(_ {
        stmt: Statement::Query(_ {
            body: ExprSet::Select(_ {
                filter.expr: Some(stmt::Expr::BinaryOp(_ {
                    *lhs: == stmt::Expr::column(val_column),
                    op: stmt::BinaryOp::Eq,
                    *rhs: stmt::Expr::Value(stmt::Value::String(== ts_text)),
                    ..
                })),
                ..
            }),
            ..
        }),
        ..
    }));

    Ok(())
}

/// Same test but with a UUID field stored as TEXT — verifies the bijection path
/// handles the common UUID-as-text encoding pattern.
#[driver_test(id(ID), requires(sql))]
pub async fn filter_uuid_stored_as_text(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,
        #[index]
        #[column(type = text)]
        val: uuid::Uuid,
    }

    let mut db = test.setup_db(models!(Foo)).await;

    let val = uuid::Uuid::new_v4();
    let created = Foo::create().val(val).exec(&mut db).await?;

    test.log().clear();

    let results = Foo::filter_by_val(val).collect::<Vec<_>>(&mut db).await?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].val, val);
    assert_eq!(results[0].id, created.id);

    // The bijection should encode the UUID as a String in the filter
    let (op, _) = test.log().pop();
    let val_text = val.to_string();
    let val_column = column(&db, "foos", "val");

    assert_struct!(&op, Operation::QuerySql(_ {
        stmt: Statement::Query(_ {
            body: ExprSet::Select(_ {
                filter.expr: Some(stmt::Expr::BinaryOp(_ {
                    *lhs: == stmt::Expr::column(val_column),
                    op: stmt::BinaryOp::Eq,
                    *rhs: stmt::Expr::Value(stmt::Value::String(== val_text)),
                    ..
                })),
                ..
            }),
            ..
        }),
        ..
    }));

    Ok(())
}
