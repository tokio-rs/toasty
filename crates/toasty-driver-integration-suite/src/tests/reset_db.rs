use crate::prelude::*;

#[driver_test(serial)]
pub async fn reset_db_and_recreate(t: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: i32,
        name: String,
    }

    // Setup and insert data
    let db = t.setup_db(models!(User)).await;
    User::create().id(1).name("Alice").exec(&db).await.unwrap();
    User::create().id(2).name("Bob").exec(&db).await.unwrap();

    // Verify data exists by key lookup
    let alice = User::get_by_id(&db, &1).await.unwrap();
    assert_eq!(alice.name, "Alice");
    let bob = User::get_by_id(&db, &2).await.unwrap();
    assert_eq!(bob.name, "Bob");

    // Reset the database
    db.reset_db().await.unwrap();

    // Re-setup (tables were dropped along with the database)
    let db = t.setup_db(models!(User)).await;

    // Verify the data is gone — lookups by known keys should return nothing
    assert_err!(User::get_by_id(&db, &1).await);
    assert_err!(User::get_by_id(&db, &2).await);
}
