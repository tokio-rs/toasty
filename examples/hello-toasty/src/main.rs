mod db;
use std::path::PathBuf;

use db::{Todo, User};

use toasty::Db;
use toasty_sqlite::Sqlite;

fn assert_sync_send<T: Send>(_: T) {}

#[tokio::main]
async fn main() {
    let schema_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("hello-toasty")
        .join("schema.toasty");

    let schema = toasty::schema::from_file(schema_file).unwrap();

    println!("{schema:#?}");

    // Use the in-memory sqlite driver
    let driver = Sqlite::in_memory();

    let db = Db::new(schema, driver).await;
    // For now, reset!s
    db.reset_db().await.unwrap();

    assert_sync_send(db::User::find_by_email("hello").first(&db));

    println!("==> let u1 = User::create()");
    let u1 = User::create()
        .name("John Doe")
        .email("john@example.com")
        .exec(&db)
        .await
        .unwrap();

    println!("==> let u2 = User::create()");
    let u2 = User::create()
        .name("Nancy Huerta")
        .email("nancy@example.com")
        .exec(&db)
        .await
        .unwrap();

    // Find by ID
    println!("==> let user = User::find_by_id(&u1.id)");
    let user = User::find_by_id(&u1.id).get(&db).await.unwrap();
    println!("USER = {user:#?}");

    // Find by email!
    println!("==> let user = User::find_by_email(&u1.email)");
    let mut user = User::find_by_email(&u1.email).get(&db).await.unwrap();
    println!("USER = {user:#?}");

    assert!(User::create()
        .name("John Dos")
        .email("john@example.com")
        .exec(&db)
        .await
        .is_err());

    user.update().name("Foo bar").exec(&db).await.unwrap();
    assert_eq!(user.name, "Foo bar");
    assert_eq!(
        User::find_by_id(&user.id).get(&db).await.unwrap().name,
        user.name
    );

    // Load the user again
    let user = User::find_by_id(&u2.id).get(&db).await.unwrap();
    println!("  reloaded -> {user:#?}");

    println!(" ~~~~~~~~~~~ CREATE TODOs ~~~~~~~~~~~~");

    // n1.todos().query();
    // n1.todos().all(&db).await.unwrap();
    let todo = u2
        .todos()
        .create()
        .title("finish toasty")
        .exec(&db)
        .await
        .unwrap();

    println!("CREATED = {todo:#?}");

    let mut todos = u2.todos().all(&db).await.unwrap();

    while let Some(todo) = todos.next().await {
        let todo = todo.unwrap();
        println!("TODO = {todo:#?}");
        println!("-> user {:?}", todo.user().find(&db).await.unwrap());
    }

    // Now, find todos by user id
    // let mut todos = db::Todo::find_by_user(&u2.id).all(&db).await.unwrap();

    // Delete user
    user.delete(&db).await.unwrap();
    assert!(User::find_by_id(&u2.id).get(&db).await.is_err());

    // Create a batch of users
    User::create_many()
        .item(User::create().email("foo@example.com").name("User Foo"))
        .item(User::create().email("bar@example.com").name("User Bar"))
        .exec(&db)
        .await
        .unwrap();

    // Lets create a new user. This time, we will batch create todos for the
    // user
    let mut user = User::create()
        .name("Ann Chovey")
        .email("ann.chovey@example.com")
        .todo(Todo::create().title("Make pizza"))
        .todo(Todo::create().title("Sleep"))
        .exec(&db)
        .await
        .unwrap();

    user.update()
        .todo(Todo::create().title("might delete later"))
        .exec(&db)
        .await
        .unwrap();

    // Get the last todo so we can unlink it
    let todos = user.todos().collect::<Vec<_>>(&db).await.unwrap();
    let len = todos.len();

    user.todos()
        .remove(todos.last().unwrap())
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(
        len - 1,
        user.todos().collect::<Vec<_>>(&db).await.unwrap().len()
    );

    println!(">>> DONE <<<");
}
