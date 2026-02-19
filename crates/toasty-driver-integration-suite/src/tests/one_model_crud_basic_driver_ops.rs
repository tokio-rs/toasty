use crate::helpers::column;
use crate::prelude::*;

use toasty_core::{
    driver::{Operation, Rows},
    stmt::{BinaryOp, Expr, ExprColumn, ExprSet, Source, Statement, Type, Value},
};

#[driver_test(id(ID))]
pub async fn basic_crud(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,
        age: i32,
    }

    let db = test.setup_db(models!(User)).await;

    // Helper to get the table ID (handles database-specific prefixes automatically)
    let user_table_id = table_id(&db, "users");
    let user_id_column = column(&db, "users", "id");

    // Clear any setup operations (from reset_db, etc.)
    test.log().clear();

    let is_sql = test.capability().sql;

    // ========== CREATE ==========
    let user = User::create().name("Alice").age(30).exec(&db).await?;

    // Check the CREATE operation
    let (op, resp) = test.log().pop();

    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Insert(_ {
            target: toasty_core::stmt::InsertTarget::Table(_ {
                table: user_table_id,
                columns.len(): 3,
                columns: == columns(&db, "users", &["id", "name", "age"]),
                ..
            }),
            source: _ {
                body: _,
                ..
            },
            ..
        }),
        // ret: None,
        ..
    }));

    if driver_test_cfg!(id_u64) && test.capability().returning_from_mutation {
        assert_struct!(op, Operation::QuerySql(_ {
            ret: Some([Type::U64]),
            last_insert_id_hack: None,
            ..
        }));

        let rows = resp.rows.collect_as_value().await?;

        // Check response
        assert_struct!(rows, Value::List([Value::Record([1])]));
    } else if driver_test_cfg!(id_u64) {
        assert_struct!(op, Operation::QuerySql(_ {
            ret: None,
            last_insert_id_hack: Some(1),
            ..
        }));

        let rows = resp.rows.collect_as_value().await?;

        // Check response
        assert_struct!(rows, Value::List([Value::Record([1])]));
    } else {
        assert_struct!(op, Operation::QuerySql(_ {
            ret: None,
            ..
        }));

        // Check response
        assert_struct!(resp, _ {
            rows: Rows::Count(1),
            ..
        });
    }

    let user_id = user.id;

    // ========== READ ==========
    let fetched = User::get_by_id(&db, &user_id).await?;
    assert_eq!(fetched.name, "Alice");
    assert_eq!(fetched.age, 30);

    // Check the READ operation
    let (op, resp) = test.log().pop();

    if is_sql {
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Query(_ {
                body: ExprSet::Select(_ {
                    source: Source::Table(_ {
                        tables: [user_table_id, ..],
                        ..
                    }),
                    filter.expr: Some(Expr::BinaryOp(_ {
                        lhs.as_expr_column_unwrap(): ExprColumn {
                            nesting: 0,
                            table: 0,
                            column: user_id_column.index,
                        },
                        op: BinaryOp::Eq,
                        rhs: _,
                        ..
                    })),
                    ..
                }),
                ..
            }),
            ret: Some(_),
            ..
        }));
    } else {
        assert_struct!(op, Operation::GetByKey(_ {
            table: user_table_id,
            keys: _,
            select.len(): 3,
            ..
        }));
    }

    assert_struct!(resp.rows, Rows::Stream(_));

    // ========== UPDATE ==========
    User::filter_by_id(user_id)
        .update()
        .age(31)
        .exec(&db)
        .await?;

    // Check the UPDATE operation
    let (op, resp) = test.log().pop();

    if is_sql {
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Update(_ {
                target: toasty_core::stmt::UpdateTarget::Table(user_table_id),
                assignments: #{ 2: _ { expr: 31, .. }},
                filter.expr: Some(Expr::BinaryOp(_ {
                    lhs.as_expr_column_unwrap(): ExprColumn {
                        nesting: 0,
                        table: 0,
                        column: user_id_column.index,
                    },
                    op: BinaryOp::Eq,
                    rhs: _,
                    ..
                })),
                ..
            }),
            ret: None,
            ..
        }));
    } else {
        assert_struct!(op, Operation::UpdateByKey(_ {
            table: user_table_id,
            filter: None,
            keys: _,
            assignments: #{ 2: _ { expr: 31, .. }},
            returning: false,
            ..
        }));
    }

    assert_struct!(resp, _ {
        rows: Rows::Count(1),
        ..
    });

    // ========== DELETE ==========
    User::filter_by_id(user_id).delete(&db).await?;

    // Check the DELETE operation
    let (op, resp) = test.log().pop();

    if is_sql {
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Delete(_ {
                from: Source::Table(_ {
                    tables: [user_table_id, ..],
                    ..
                }),
                filter.expr: Some(Expr::BinaryOp(_ {
                    lhs.as_expr_column_unwrap(): ExprColumn {
                        nesting: 0,
                        table: 0,
                        column: user_id_column.index,
                    },
                    op: BinaryOp::Eq,
                    rhs: _,
                    ..
                })),
                ..
            }),
            ..
        }));
    } else {
        assert_struct!(op, Operation::DeleteByKey(_ {
            table: user_table_id,
            filter: None,
            keys: _,
            ..
        }));
    }

    // Check response
    assert_struct!(resp, _ {
        rows: Rows::Count(1),
        ..
    });

    // ========== VERIFY LOG IS EMPTY ==========
    assert!(test.log().is_empty(), "Log should be empty");
    Ok(())
}
