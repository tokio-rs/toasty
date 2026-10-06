#![cfg(feature = "turso-serverless")]

//! Regression tests against a disposable libSQL Turso Cloud database.
//! Set `TOASTY_TEST_TURSO_LIBSQL_URL` and `TOASTY_TEST_TURSO_LIBSQL_TOKEN`, then run
//! `cargo test -p tests --features turso-serverless --test turso_libsql -- --ignored`.

use toasty_core::driver::operation::TransactionMode;
use toasty_driver_turso::Turso;

#[derive(Debug, toasty::Model)]
struct Parent {
    #[key]
    id: i64,
    #[has_many]
    children: toasty::Deferred<Vec<Child>>,
}

#[derive(Debug, toasty::Model)]
struct Child {
    #[key]
    id: i64,
    #[index]
    parent_id: i64,
    #[belongs_to(key = parent_id, references = id)]
    parent: toasty::Deferred<Parent>,
}

#[derive(Debug, toasty::Model)]
struct VersionedItem {
    #[key]
    id: i64,
    name: String,
    #[version]
    version: u64,
}

#[tokio::test]
#[ignore = "requires a disposable libSQL Turso Cloud database"]
async fn implicit_transactions_work_on_libsql() {
    let url = std::env::var("TOASTY_TEST_TURSO_LIBSQL_URL").expect("set the libSQL database URL");
    let token =
        std::env::var("TOASTY_TEST_TURSO_LIBSQL_TOKEN").expect("set the libSQL database token");
    let driver = Turso::new(url)
        .unwrap()
        .with_auth_token(token)
        .with_transaction_mode(TransactionMode::Deferred);
    let mut db = toasty::Db::builder()
        .models(toasty::models!(Parent, Child, VersionedItem))
        .build(driver)
        .await
        .unwrap();
    db.push_schema().await.unwrap();

    // Explicit deferred transactions work on libSQL even without a driver default.
    // Seed through one to isolate the implicit transaction used by the delete.
    let mut tx = db
        .transaction_builder()
        .mode(TransactionMode::Deferred)
        .begin()
        .await
        .unwrap();
    let parent = toasty::create!(Parent { id: 1 })
        .exec(&mut tx)
        .await
        .unwrap();
    toasty::create!(Child {
        id: 1,
        parent_id: 1
    })
    .exec(&mut tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(parent.children().exec(&mut db).await.unwrap().len(), 1);

    parent.delete().exec(&mut db).await.unwrap();
    assert!(Parent::all().exec(&mut db).await.unwrap().is_empty());
    assert!(Child::all().exec(&mut db).await.unwrap().is_empty());

    // A versioned update takes the standalone read-modify-write path.
    let mut item = toasty::create!(VersionedItem {
        id: 1,
        name: "before"
    })
    .exec(&mut db)
    .await
    .unwrap();
    item.update().name("after").exec(&mut db).await.unwrap();
    let reloaded = VersionedItem::get_by_id(&mut db, &1).await.unwrap();
    assert_eq!(reloaded.name, "after");
    assert_eq!(reloaded.version, 2);

    let tx = db.transaction().await.unwrap();
    tx.rollback().await.unwrap();
}
