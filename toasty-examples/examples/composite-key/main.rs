mod db;
use std::path::PathBuf;

use db::User;

use toasty::Db;

#[cfg(feature = "sqlite")]
use toasty_sqlite::Sqlite;

#[cfg(feature = "dynamodb")]
use toasty_dynamodb::DynamoDB;

#[tokio::main]
async fn main() {
    let schema_file: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("hello-toasty")
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

    println!("==> let user = User::create()");
    let user = User::create()
        .name("John Doe")
        .email("john@example.com")
        .exec(&db)
        .await
        .unwrap();

    println!(" ~~~~~~~~~~~ CREATE TODOs ~~~~~~~~~~~~");

    for (i, title) in ["finish toasty", "retire", "play golf"].iter().enumerate() {
        let todo = user
            .todos()
            .create()
            .title(*title)
            .order(i as i64)
            .exec(&db)
            .await
            .unwrap();

        println!("CREATED = {todo:#?}");
    }

    // let mut todos = user.todos().all(&db).await.unwrap();

    // while let Some(todo) = todos.next().await {
    //     let todo = todo.unwrap();
    //     println!("TODO = {:#?}", todo);
    // }

    // Query a user's todos
    println!("====================");
    println!("--- QUERY ---");
    println!("====================");

    let mut todos = user
        .todos()
        .query(db::Todo::ORDER.eq(1))
        .all(&db)
        .await
        .unwrap();

    while let Some(todo) = todos.next().await {
        let todo = todo.unwrap();
        println!("TODO = {todo:#?}");
    }

    println!(">>> DONE <<<");
}
