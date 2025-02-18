mod db;
use std::path::PathBuf;

use db::*;

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

    println!("==> let u1 = User::create()");
    let u1 = User::create()
        .name("John Doe")
        .email("john@example.com")
        .exec(&db)
        .await?;

    println!(" -> u1 = {u1:#?}");

    println!("==> let u2 = User::create()");
    let u2 = User::create()
        .name("Jane doe")
        .email("jane@example.com")
        .exec(&db)
        .await?;
    println!(" -> u2 = {u2:#?}");

    let p1 = u1.packages().create().name("tokio").exec(&db).await?;

    println!("==> Package::find_by_user_and_id(&u1, &p1.id)");
    let package = Package::get_by_user_id_and_id(&db, &u1.id, &p1.id).await?;

    println!("{package:#?}");

    println!("==> u1.packages().all(&db)");
    let packages = u1.packages().all(&db).await?.collect::<Vec<_>>().await;
    println!("packages = {packages:#?}");

    Ok(())
}
