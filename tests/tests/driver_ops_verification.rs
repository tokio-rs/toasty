use tests::{models, tests, DbTest};
use toasty::stmt::Id;

async fn test_driver_ops_logging(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<User>,

        name: String,
        age: i32,
    }

    let db = test.setup_db(models!(User)).await;

    // Create a user - this should generate an Insert operation
    let user = User::create()
        .name("Alice")
        .age(30)
        .exec(&db)
        .await
        .unwrap();

    // Query the user - this should generate a GetByKey or QueryPk operation
    let fetched = User::get_by_id(&db, &user.id).await.unwrap();
    assert_eq!(fetched.name, "Alice");
    assert_eq!(fetched.age, 30);

    // Update the user - this should generate an UpdateByKey operation
    User::filter_by_id(&user.id)
        .update()
        .age(31)
        .exec(&db)
        .await
        .unwrap();

    // Delete the user - this should generate a DeleteByKey operation
    User::filter_by_id(&user.id).delete(&db).await.unwrap();

    // Use the ExecLog API for assertions
    let log = test.log();

    // Basic assertions - we should have at least 4 operations
    assert!(
        log.len() >= 4,
        "Expected at least 4 operations (insert, get, update, delete), got {}",
        log.len()
    );

    // Since operations might be lowered to SQL, we check for either specific ops or SQL
    assert!(
        log.has_insert() || log.has_query_sql(),
        "Expected to find an Insert or QuerySql operation"
    );
    assert!(
        log.has_get_by_key() || log.has_query_sql(),
        "Expected to find a GetByKey or QuerySql operation"
    );
    assert!(
        log.has_update_by_key() || log.has_query_sql(),
        "Expected to find an UpdateByKey or QuerySql operation"
    );
    assert!(
        log.has_delete_by_key() || log.has_query_sql(),
        "Expected to find a DeleteByKey or QuerySql operation"
    );
}

tests!(test_driver_ops_logging,);