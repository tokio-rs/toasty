use tests::{models, tests, DbTest};
use toasty::stmt::Id;
use toasty_core::{
    driver::{Operation, Rows},
    schema::db::TableId,
    stmt::Value,
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
    
    // Clear any setup operations (from reset_db, etc.)
    test.log().clear();

    // ========== CREATE ==========
    let user = User::create()
        .name("Alice")
        .age(30)
        .exec(&db)
        .await
        .unwrap();

    // Check the CREATE operation
    let (create_op, create_resp) = test.log().pop().expect("Expected create operation");
    
    // Check the operation type - both SQL and some KV stores use QuerySql
    match create_op {
        Operation::QuerySql(query_sql) => {
            // The statement should be an INSERT
            assert!(
                matches!(&query_sql.stmt, toasty_core::stmt::Statement::Insert(_)),
                "Expected Insert statement for CREATE"
            );
        }
        Operation::Insert(insert) => {
            // Some drivers use Insert directly
            assert!(
                matches!(&insert.stmt, toasty_core::stmt::Statement::Insert(_)),
                "Expected Insert statement"
            );
        }
        _ => panic!("Expected QuerySql or Insert operation, got {:?}", create_op),
    }

    // Check response has row count
    match create_resp.rows {
        Rows::Count(count) => assert_eq!(count, 1, "Insert should affect 1 row"),
        Rows::Values(_) => panic!("Insert should return count, not values"),
    }

    let user_id = user.id.clone();

    // ========== READ ==========
    let fetched = User::get_by_id(&db, &user_id).await.unwrap();
    assert_eq!(fetched.name, "Alice");
    assert_eq!(fetched.age, 30);

    // Check the READ operation
    let (read_op, read_resp) = test.log().pop().expect("Expected read operation");

    match read_op {
        Operation::QuerySql(query_sql) => {
            // The statement should be a SELECT (Query)
            assert!(
                matches!(&query_sql.stmt, toasty_core::stmt::Statement::Query(_)),
                "Expected Query statement for READ"
            );
        }
        Operation::GetByKey(get) => {
            // Some drivers use GetByKey directly
            assert_eq!(get.table, TableId(0), "Should get from User table");
            assert_eq!(get.keys.len(), 1, "Should have 1 key");
            match &get.keys[0] {
                Value::String(id) => {
                    assert_eq!(id, user_id.to_string().as_str(), "Key should match user ID");
                }
                _ => panic!("Expected String key"),
            }
        }
        _ => panic!("Expected QuerySql or GetByKey operation, got {:?}", read_op),
    }

    // Check response has values
    match read_resp.rows {
        Rows::Values(_) => {
            // Response contains a value stream with the user data
        }
        Rows::Count(_) => panic!("Read should return values, not count"),
    }

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
            // The statement should be an UPDATE
            assert!(
                matches!(&query_sql.stmt, toasty_core::stmt::Statement::Update(_)),
                "Expected Update statement for UPDATE"
            );
        }
        Operation::UpdateByKey(update) => {
            // Some drivers use UpdateByKey directly
            assert_eq!(update.table, TableId(0), "Should update User table");
            assert_eq!(update.keys.len(), 1, "Should have 1 key");
            match &update.keys[0] {
                Value::String(id) => {
                    assert_eq!(id, user_id.to_string().as_str(), "Key should match user ID");
                }
                _ => panic!("Expected String key"),
            }
            // The update operation should have assignments (we updated age)
        }
        _ => panic!("Expected QuerySql or UpdateByKey operation, got {:?}", update_op),
    }

    // Check response - most databases return count, but some might return values
    match update_resp.rows {
        Rows::Count(count) => assert_eq!(count, 1, "Update should affect 1 row"),
        Rows::Values(_) => {
            // Some drivers (like DynamoDB) might return the updated values
        }
    }

    // ========== DELETE ==========
    User::filter_by_id(&user_id).delete(&db).await.unwrap();

    // Check the DELETE operation
    let (delete_op, delete_resp) = test.log().pop().expect("Expected delete operation");

    match delete_op {
        Operation::QuerySql(query_sql) => {
            // The statement should be a DELETE
            assert!(
                matches!(&query_sql.stmt, toasty_core::stmt::Statement::Delete(_)),
                "Expected Delete statement for DELETE"
            );
        }
        Operation::DeleteByKey(delete) => {
            // Some drivers use DeleteByKey directly
            assert_eq!(delete.table, TableId(0), "Should delete from User table");
            assert_eq!(delete.keys.len(), 1, "Should have 1 key");
            match &delete.keys[0] {
                Value::String(id) => {
                    assert_eq!(id, user_id.to_string().as_str(), "Key should match user ID");
                }
                _ => panic!("Expected String key"),
            }
        }
        _ => panic!("Expected QuerySql or DeleteByKey operation, got {:?}", delete_op),
    }

    // Check response has row count
    match delete_resp.rows {
        Rows::Count(count) => assert_eq!(count, 1, "Delete should affect 1 row"),
        Rows::Values(_) => panic!("Delete should return count, not values"),
    }

    // ========== VERIFY LOG IS EMPTY ==========
    assert!(
        test.log().is_empty(),
        "Log should be empty after all operations, but has {} entries",
        test.log().len()
    );
}

tests!(basic_crud,);