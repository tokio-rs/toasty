mod db;

use toasty::Db;
use toasty_dynamodb::DynamoDB;

#[tokio::main]
async fn main() {
    let schema_file = std::path::Path::new(file!())
        .parent()
        .unwrap()
        .join("../schema.toasty");

    let schema = toasty::schema::from_file(schema_file).unwrap();
    let driver = DynamoDB::from_env().await.unwrap();
    let db = Db::new(schema, driver).await;

    // For now, reset!s
    db.reset_db().await.unwrap();

    // Create a user without a profile
    let user = db::User::create().name("John Doe").exec(&db).await.unwrap();

    println!("user = {user:#?}");
    println!("profile: {:#?}", user.profile().get(&db).await);
}
