mod db;
use std::path::PathBuf;

use db::*;

use toasty::Db;
use toasty_sqlite::Sqlite;

#[tokio::main]
async fn main() {
    let schema_file: PathBuf = std::env::current_dir().unwrap().join("schema.toasty");
    let schema = toasty::schema::from_file(schema_file).unwrap();

    // Use the in-memory sqlite driver
    let driver = Sqlite::in_memory();

    let db = Db::new(schema, driver).await;
    // For now, reset!s
    db.reset_db().await.unwrap();

    println!("==> let u1 = User::create()");
    let u1 = User::create()
        .name("John Doe")
        .email("john@example.com")
        .exec(&db)
        .await
        .unwrap();

    println!(" -> u1 = {u1:#?}");

    println!("==> let u2 = User::create()");
    let u2 = User::create()
        .name("Jane doe")
        .email("jane@example.com")
        .exec(&db)
        .await
        .unwrap();
    println!(" -> u2 = {u2:#?}");

    let p1 = u1
        .packages()
        .create()
        .name("tokio")
        .exec(&db)
        .await
        .unwrap();

    println!("==> Package::find_by_user_and_id(&u1, &p1.id)");
    let package = Package::find_by_user_id_and_id(&u1.id, &p1.id)
        .get(&db)
        .await
        .unwrap();

    println!("{package:#?}");

    println!("==> u1.packages().all(&db)");
    let packages = u1
        .packages()
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await;
    println!("packages = {packages:#?}");

    // Find the user again, this should not include the package
    println!("==> User::find_by_id(&u1.id)");
    let users = User::find_by_id(&u1.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(1, users.len());

    for user in users {
        println!("USER = {user:#?}");
    }
}
