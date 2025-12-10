use assert_struct::assert_struct;
use tests::{prelude::*, stmt::Any};
use toasty::stmt::Id;
use toasty_core::{
    driver::{Operation, Rows},
    stmt::{BinaryOp, Expr, ExprColumn, ExprSet, Source, Statement},
};

async fn basic_crud(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<User>,

        name: String,
        age: i32,
    }

    let db = test.setup_db(models!(User)).await;

    // Helper to get the table ID (handles database-specific prefixes automatically)
    let user_table_id = table_id(&db, "users");
    let user_id_column_index = column(&db, "users", "id").index;

    // Clear any setup operations (from reset_db, etc.)
    test.log().clear();

    let is_sql = test.capability().sql;

    // ========== CREATE ==========
    let user = User::create()
        .name("Alice")
        .age(30)
        .exec(&db)
        .await
        .unwrap();

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
                body: =~ [(Any, "Alice", 30)],
                ..
            },
            ..
        }),
        ret: None,
        ..
    }));

    // Check response
    assert_struct!(resp, _ {
        rows: Rows::Count(1),
        ..
    });

    let user_id = user.id.to_string();

    // ========== READ ==========
    let fetched = User::get_by_id(&db, &user_id).await.unwrap();
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
                            column: user_id_column_index,
                        },
                        op: BinaryOp::Eq,
                        *rhs: == user_id,
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
            keys: [=~ (&user_id,)],
            select.len(): 3,
            ..
        }));
    }

    assert_struct!(resp.rows, Rows::Stream(
        0.buffered(): [
            =~ (user_id.clone(), "Alice", 30),
        ],
    ));

    // ========== UPDATE ==========
    User::filter_by_id(&user_id)
        .update()
        .age(31)
        .exec(&db)
        .await
        .unwrap();

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
                        column: user_id_column_index,
                    },
                    op: BinaryOp::Eq,
                    *rhs: == user_id,
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
            keys: [=~ (&user_id,)],
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
    User::filter_by_id(&user_id).delete(&db).await.unwrap();

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
                        column: user_id_column_index,
                    },
                    op: BinaryOp::Eq,
                    *rhs: == user_id,
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
            keys: [=~ (&user_id,)],
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
}

tests!(basic_crud,);
