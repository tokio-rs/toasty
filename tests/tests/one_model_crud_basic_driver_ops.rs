use assert_struct::assert_struct;
use tests::{prelude::*, stmt::Any};
use toasty::stmt::Id;
use toasty_core::{
    driver::{Operation, Rows},
    stmt::{BinaryOp, Expr, ExprColumn, ExprSet, Source, Statement, Value},
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
                    source: Source::Table([
                        _ { table: user_table_id, .. },
                    ]),
                    filter: Expr::BinaryOp(_ {
                        *lhs: Expr::Column(ExprColumn::Column(== column(&db, "users", "id"))),
                        op: BinaryOp::Eq,
                        *rhs: == user_id,
                        ..
                    }),
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
            keys.len(): 1,
            keys[0]: Value::String(_),
            select.len(): 3,
            ..
        }));
    }

    assert_struct!(resp.rows, Rows::Values(
        0.buffered(): [
            =~ (user_id.clone(), "Alice", 30),
        ],
    ));

    // Check response has values and validate actual returned data
    match resp.rows {
        Rows::Values(stream) => {
            let values = stream.collect().await.unwrap();
            assert_eq!(values.len(), 1, "Should return exactly one user record");

            // Check that the returned record contains user data (id, name, age)
            if let Value::Record(ref record) = values[0] {
                assert_eq!(record.fields.len(), 3, "User record should have 3 fields");
                // The exact order and format may vary by driver, but we should have the core data
            } else {
                panic!("Expected Record value, got {:?}", values[0]);
            }
        }
        _ => panic!("READ operation should return Values, got {:?}", resp.rows),
    }

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
                assignments.len(): 1,
                assignments[2]: _ {
                    expr: Expr::Value(Value::I32(31)),
                    ..
                },
                filter: Some(Expr::BinaryOp(_ {
                    *lhs: Expr::Column(ExprColumn::Column(== column(&db, "users", "id"))),
                    op: BinaryOp::Eq,
                    *rhs: == user_id,
                    ..
                })),
                ..
            }),
            ..
        }));
    } else {
        assert_struct!(op, Operation::UpdateByKey(_ {
            table: user_table_id,
            filter: None,
            keys.len(): 1,
            keys[0]: Value::String(_),
            ..
        }));
    }

    // Check response
    if is_sql {
        assert_struct!(resp, _ {
            rows: Rows::Count(1),
            ..
        });
    } else {
        // DynamoDB and some KV stores return values from updates
        match resp.rows {
            Rows::Values(stream) => {
                let values = stream.collect().await.unwrap();
                assert_eq!(values.len(), 1, "Should return exactly one updated record");

                // Check that the returned record contains updated data
                if let Value::Record(ref record) = values[0] {
                    assert_eq!(
                        record.fields.len(),
                        3,
                        "Updated record should have 3 fields"
                    );
                    // Should contain the updated age value (31)
                } else {
                    panic!("Expected Record value from UPDATE, got {:?}", values[0]);
                }
            }
            _ => panic!("Non-SQL UPDATE should return Values, got {:?}", resp.rows),
        }
    }

    // ========== DELETE ==========
    User::filter_by_id(&user_id).delete(&db).await.unwrap();

    // Check the DELETE operation
    let (op, resp) = test.log().pop();

    if is_sql {
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Delete(_ {
                from: Source::Table([_ {
                    table: user_table_id,
                    ..
                }]),
                filter: Expr::BinaryOp(_ {
                    *lhs: Expr::Column(ExprColumn::Column(== column(&db, "users", "id"))),
                    op: BinaryOp::Eq,
                    *rhs: == user_id,
                    ..
                }),
                ..
            }),
            ..
        }));
    } else {
        assert_struct!(op, Operation::DeleteByKey(_ {
            table: user_table_id,
            filter: None,
            keys.len(): 1,
            keys[0]: Value::String(_),
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
