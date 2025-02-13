use std::path::PathBuf;

use postgres::NoTls;
use toasty::Db;
use toasty_pgsql::PostgreSQL;

mod db;

use db::Todo;
use db::User;

#[tokio::main]
async fn main() -> toasty::Result<()> {
    //=========================//
    // Connect to the database //
    //=========================//

    let url = std::env::var("DATABASE_URL")
        .expect("`DATABASE_URL` environment variable is required for this example");
    let driver = PostgreSQL::connect(&url, NoTls).await?;

    //===============//
    // Toasty schema //
    //===============//

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema.toasty");
    let schema = toasty::schema::from_file(path)?;

    let db = Db::new(schema, driver).await?;
    db.reset_db().await?;

    //==========================//
    // Run the database queries //
    //==========================//

    println!("==> let u1 = User::create()");
    let u1 = User::create()
        .name("John Doe")
        .email("john@example.com")
        .exec(&db)
        .await?;

    println!("==> let u2 = User::create()");
    let u2 = User::create()
        .name("Nancy Huerta")
        .email("nancy@example.com")
        .exec(&db)
        .await?;

    println!("==> let user = User::find_by_id(&u1.id)");
    let user = User::find_by_id(&u1.id).get(&db).await?;
    println!("USER = {user:#?}");

    println!("==> let user = User::find_by_email(&u1.email)");
    let mut user = User::find_by_email(&u1.email).get(&db).await?;
    println!("USER = {user:#?}");

    assert!(User::create()
        .name("John Dos")
        .email("john@example.com")
        .exec(&db)
        .await
        .is_err());

    user.update().name("Foo bar").exec(&db).await?;
    assert_eq!(user.name, "Foo bar");
    assert_eq!(User::find_by_id(&user.id).get(&db).await?.name, user.name);

    // Load the user again
    let user = User::find_by_id(&u1.id).get(&db).await?;
    println!("  reloaded, notice change to the user's name -> {user:#?}");

    println!(" ~~~~~~~~~~~ CREATE TODOs ~~~~~~~~~~~~");

    let todo = u2.todos().create().title("finish toasty").exec(&db).await?;

    println!("CREATED = {todo:#?}");

    let mut todos = u2.todos().all(&db).await?;

    while let Some(todo) = todos.next().await {
        let todo = todo?;
        println!("TODO = {todo:#?}");
        println!("-> user {:?}", todo.user().find(&db).await?);
    }

    // Delete user
    let user = User::find_by_id(&u2.id).get(&db).await?;
    user.delete(&db).await?;
    assert!(User::find_by_id(&u2.id).get(&db).await.is_err());

    // Create a batch of users
    User::create_many()
        .item(User::create().email("foo@example.com").name("User Foo"))
        .item(User::create().email("bar@example.com").name("User Bar"))
        .exec(&db)
        .await?;

    // Lets create a new user. This time, we will batch create todos for the
    // user
    let mut user = User::create()
        .name("Ann Chovey")
        .email("ann.chovey@example.com")
        .todo(Todo::create().title("Make pizza"))
        .todo(Todo::create().title("Sleep"))
        .exec(&db)
        .await?;

    user.update()
        .todo(Todo::create().title("might delete later"))
        .exec(&db)
        .await?;

    // Get the last todo so we can unlink it
    let todos = user.todos().collect::<Vec<_>>(&db).await?;
    let len = todos.len();

    user.todos().remove(todos.last().unwrap()).exec(&db).await?;

    assert_eq!(len - 1, user.todos().collect::<Vec<_>>(&db).await?.len());

    println!(">>> DONE <<<");

    Ok(())
}
