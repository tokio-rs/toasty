mod db;
use std::path::PathBuf;

use toasty::Db;

#[cfg(feature = "sqlite")]
use toasty_sqlite::Sqlite;

#[cfg(feature = "dynamodb")]
use toasty_dynamodb::DynamoDB;

#[tokio::main]
async fn main() {
    let schema_file: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("EXAMPLE_NAME")
        .join("schema.toasty");

    let schema = toasty::schema::from_file(schema_file).unwrap();

    println!("{schema:#?}");

    #[cfg(feature = "sqlite")]
    let driver = Sqlite::in_memory();

    #[cfg(feature = "dynamodb")]
    let driver = DynamoDB::from_env().await.unwrap();

    let db = Db::new(schema, driver).await;

    // For now, reset!s
    db.reset_db().await.unwrap();
}