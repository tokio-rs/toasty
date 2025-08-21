use assert_struct::assert_struct;
use tests::{column, columns, models, table_id, tests, DbTest};
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
    let (create_op, create_resp) = test.log().pop().expect("Expected create operation");

    match create_op {
        Operation::QuerySql(query_sql) => {
            // Comprehensive CREATE validation: statement, target, columns, values, and return type
            use tests::expr::Any;

            assert_struct!(query_sql, _ {
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
            });
        }
        _ => panic!("Unexpected operation type for CREATE: {:?}", create_op),
    }

    // Check response
    assert_struct!(create_resp, _ {
        rows: Rows::Count(1),
        ..
    });

    let user_id = user.id.clone();
    let user_id_string = user_id.to_string();

    // ========== READ ==========
    let fetched = User::get_by_id(&db, &user_id).await.unwrap();
    assert_eq!(fetched.name, "Alice");
    assert_eq!(fetched.age, 30);

    // Check the READ operation
    let (read_op, read_resp) = test.log().pop().expect("Expected read operation");

    match read_op {
        Operation::QuerySql(query_sql) => {
            if !is_sql {
                panic!("Non-SQL drivers (except DynamoDB) should not use QuerySql for reads");
            }

            // Comprehensive SELECT query validation
            assert_struct!(query_sql, _ {
                stmt: Statement::Query(_ {
                    body: ExprSet::Select(_ {
                        source: Source::Table([
                            _ { table: user_table_id, .. },
                        ]),
                        filter: Expr::BinaryOp(_ {
                            *lhs: Expr::Column(ExprColumn::Column(== column(&db, "users", "id"))),
                            op: BinaryOp::Eq,
                            *rhs: =~ user_id_string,
                            ..
                        }),
                        ..
                    }),
                    ..
                }),
                ret: Some(_),
                ..
            });

            // Extract and validate specific details
            let Statement::Query(query) = &query_sql.stmt else {
                unreachable!()
            };
            let ExprSet::Select(select) = &query.body else {
                unreachable!()
            };
            let Expr::BinaryOp(bin_op) = &select.filter else {
                unreachable!()
            };

            // Validate filter details
            assert!(bin_op.op.is_eq());
            assert!(
                matches!(&*bin_op.lhs, Expr::Column(ExprColumn::Column(col_id)) if *col_id == columns(&db, "users", &["id"])[0])
            );
            assert!(
                matches!(&*bin_op.rhs, Expr::Value(Value::String(ref id)) if id == &user_id_string)
            );
            assert_eq!(query_sql.ret.as_ref().unwrap().len(), 3);
        }
        Operation::GetByKey(get) => {
            if is_sql {
                panic!("SQL drivers should never receive GetByKey operation");
            }

            assert_struct!(get, _ {
                table: user_table_id,
                keys.len(): 1,
                keys[0]: Value::String(_),
                select.len(): 3,
                ..
            });
        }
        Operation::Insert(_) | Operation::UpdateByKey(_) | Operation::DeleteByKey(_) => {
            panic!("Invalid operation type for READ: {:?}", read_op);
        }
        _ => panic!("Unexpected operation type for READ: {:?}", read_op),
    }

    // Check response has values
    assert_struct!(read_resp, _ {
        rows: Rows::Values(_),
        ..
    });

    // ========== UPDATE ==========
    User::filter_by_id(&user_id)
        .update()
        .age(31)
        .exec(&db)
        .await
        .unwrap();

    // Check the UPDATE operation
    let (update_op, update_resp) = test.log().pop().expect("Expected update operation");

    match update_op {
        Operation::QuerySql(query_sql) => {
            if !is_sql {
                // DynamoDB also uses QuerySql
                // But pure KV stores shouldn't
            }

            // Comprehensive UPDATE validation: statement type, target, assignments, and values
            assert_struct!(query_sql, _ {
                stmt: Statement::Update(_ {
                    target: toasty_core::stmt::UpdateTarget::Table(user_table_id),
                    assignments.len(): 1,
                    assignments[2]: _ {
                        expr: Expr::Value(Value::I32(31)),
                        ..
                    },
                    ..
                }),
                ..
            });

            // Extract update for additional checks
            let Statement::Update(update) = &query_sql.stmt else {
                unreachable!()
            };

            // Check the WHERE clause
            let Some(Expr::BinaryOp(bin_op)) = &update.filter else {
                panic!("Expected BinaryOp filter");
            };

            assert!(bin_op.op.is_eq(), "Should use equality operator");

            // Check LHS is the ID column
            let Expr::Column(ExprColumn::Column(col_id)) = &*bin_op.lhs else {
                panic!("Expected Column in filter LHS");
            };
            assert_eq!(
                *col_id,
                columns(&db, "users", &["id"])[0],
                "Should filter by ID column"
            );

            // Check RHS is the user ID
            let Expr::Value(Value::String(id)) = &*bin_op.rhs else {
                panic!("Expected String value in filter RHS");
            };
            assert_eq!(id, &user_id_string, "Filter should use correct user ID");

            // Check condition and returning
            assert!(
                update.condition.is_none(),
                "Simple update should not have condition"
            );
            assert!(
                update.returning.is_none(),
                "Update should not have RETURNING clause"
            );
        }
        Operation::UpdateByKey(update) => {
            if is_sql {
                panic!("SQL drivers should never receive UpdateByKey operation");
            }

            assert_struct!(update, _ {
                table: user_table_id,
                filter: None,
                keys.len(): 1,
                keys[0]: Value::String(_),
                ..
            });
        }
        Operation::Insert(_) => {
            panic!("Insert should never be used for UPDATE operations");
        }
        Operation::GetByKey(_) => {
            panic!("GetByKey should never be used for UPDATE operations");
        }
        Operation::DeleteByKey(_) => {
            panic!("DeleteByKey should never be used for UPDATE operations");
        }
        _ => panic!("Unexpected operation type for UPDATE: {:?}", update_op),
    }

    // Check response - can be either count or values depending on driver
    match update_resp.rows {
        Rows::Count(1) => {} // Expected for SQL drivers
        Rows::Values(_) => {
            // DynamoDB and some KV stores return values from updates
            if is_sql {
                panic!("SQL databases should return count from UPDATE, not values");
            }
        }
        Rows::Count(count) => panic!("Update should affect 1 row, got {}", count),
    }

    // ========== DELETE ==========
    User::filter_by_id(&user_id).delete(&db).await.unwrap();

    // Check the DELETE operation
    let (delete_op, delete_resp) = test.log().pop().expect("Expected delete operation");

    match delete_op {
        Operation::QuerySql(query_sql) => {
            if !is_sql {
                // DynamoDB also uses QuerySql
            }

            // Verify it's a DELETE statement
            let Statement::Delete(delete) = &query_sql.stmt else {
                panic!("Expected Delete statement, got {:?}", query_sql.stmt);
            };

            // Check we're deleting from User table
            let Source::Table(tables) = &delete.from else {
                panic!("Expected Table source");
            };

            assert_eq!(tables.len(), 1, "Should delete from 1 table");
            assert_struct!(tables[0], _ {
                table: toasty_core::stmt::TableRef::Table(_),
                ..
            });

            // Verify it's the correct table
            if let toasty_core::stmt::TableRef::Table(table_id) = &tables[0].table {
                assert_eq!(*table_id, user_table_id, "Should delete from User table");
            }

            // Check the WHERE clause
            let Expr::BinaryOp(bin_op) = &delete.filter else {
                panic!("Expected BinaryOp filter");
            };

            assert!(bin_op.op.is_eq(), "Should use equality operator");

            // Check LHS is the ID column
            let Expr::Column(ExprColumn::Column(col_id)) = &*bin_op.lhs else {
                panic!("Expected Column in filter LHS");
            };
            assert_eq!(
                *col_id,
                columns(&db, "users", &["id"])[0],
                "Should filter by ID column"
            );

            // Check RHS is the user ID
            let Expr::Value(Value::String(id)) = &*bin_op.rhs else {
                panic!("Expected String value in filter RHS");
            };
            assert_eq!(id, &user_id_string, "Filter should use correct user ID");

            // Check returning (should be none)
            assert!(
                delete.returning.is_none(),
                "Delete should not have RETURNING clause"
            );
        }
        Operation::DeleteByKey(delete) => {
            if is_sql {
                panic!("SQL drivers should never receive DeleteByKey operation");
            }

            assert_struct!(delete, _ {
                table: user_table_id,
                filter: None,
                keys.len(): 1,
                keys[0]: Value::String(_),
                ..
            });
        }
        Operation::Insert(_) | Operation::GetByKey(_) | Operation::UpdateByKey(_) => {
            panic!("Invalid operation type for delete: {:?}", delete_op);
        }
        _ => panic!("Unexpected operation type for DELETE: {:?}", delete_op),
    }

    // Check response
    assert_struct!(delete_resp, _ {
        rows: Rows::Count(1),
        ..
    });

    // ========== VERIFY LOG IS EMPTY ==========
    assert!(
        test.log().is_empty(),
        "Log should be empty after all operations, but has {} entries",
        test.log().len()
    );
}

tests!(basic_crud,);
