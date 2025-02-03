mod db;

use std::path::PathBuf;

use toasty::Db;
use toasty_sqlite::Sqlite;

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let schema_file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema.toasty");

    let schema = toasty::schema::from_file(schema_file)?;

    // Use the in-memory sqlite driver
    let driver = Sqlite::in_memory();

    let db = Db::new(schema, driver).await?;

    // For now, reset!s
    db.reset_db().await?;

    // Create a user without a profile
    let user = db::User::create().name("John Doe").exec(&db).await?;

    println!("user = {user:#?}");
    println!("profile: {:#?}", user.profile().get(&db).await);

    Ok(())
}
