mod db;

use std::path::PathBuf;

use toasty::Db;
use toasty_sqlite::Sqlite;

#[tokio::main]
async fn main() {
    let schema_file = [file!(), "..", "..", "schema.toasty"]
        .iter()
        .collect::<PathBuf>()
        .canonicalize()
        .unwrap();

    let schema = toasty::schema::from_file(schema_file).unwrap();

    // Use the in-memory sqlite driver
    let driver = Sqlite::in_memory();

    let db = Db::new(schema, driver).await;

    // For now, reset!s
    db.reset_db().await.unwrap();

    // Create a user without a profile
    let user = db::User::create().name("John Doe").exec(&db).await.unwrap();

    println!("user = {user:#?}");
    println!("profile: {:#?}", user.profile().get(&db).await);
}
