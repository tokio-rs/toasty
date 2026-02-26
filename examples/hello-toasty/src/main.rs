#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,

    #[unique]
    email: String,

    #[has_many]
    todos: toasty::HasMany<Todo>,

    moto: Option<String>,
}

#[derive(Debug, toasty::Model)]
struct Todo {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[index]
    user_id: uuid::Uuid,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,

    title: String,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let mut db = toasty::Db::builder()
        .register::<User>()
        .register::<Todo>()
        .connect(
            std::env::var("TOASTY_CONNECTION_URL")
                .as_deref()
                .unwrap_or("sqlite::memory:"),
        )
        .await?;

    // For now, reset!s
    db.push_schema().await?;

    println!("==> let u1 = User::create()");
    let u1 = User::create()
        .name("John Doe")
        .email("john@example.com")
        .exec(&mut db)
        .await?;

    println!("==> let u2 = User::create()");
    let u2 = User::create()
        .name("Nancy Huerta")
        .email("nancy@example.com")
        .exec(&mut db)
        .await?;

    // Find by ID
    println!("==> let user = User::find_by_id(&u1.id)");
    let user = User::get_by_id(&mut db, &u1.id).await?;
    println!("USER = {user:#?}");

    // Find by email!
    println!("==> let user = User::find_by_email(&u1.email)");
    let mut user = User::get_by_email(&mut db, &u1.email).await?;
    println!("USER = {user:#?}");

    assert!(User::create()
        .name("John Dos")
        .email("john@example.com")
        .exec(&mut db)
        .await
        .is_err());

    user.update().name("Foo bar").exec(&mut db).await?;
    assert_eq!(user.name, "Foo bar");
    assert_eq!(User::get_by_id(&mut db, &user.id).await?.name, user.name);

    // Load the user again
    let user = User::get_by_id(&mut db, &u1.id).await?;
    println!("  reloaded, notice change to the user's name -> {user:#?}");

    println!(" ~~~~~~~~~~~ CREATE TODOs ~~~~~~~~~~~~");

    let todo = u2.todos().create().title("finish toasty").exec(&mut db).await?;

    println!("CREATED = {todo:#?}");

    let mut todos = u2.todos().all(&mut db).await?;

    while let Some(todo) = todos.next().await {
        let todo = todo?;
        println!("TODO; title={:?}", todo.title);
        println!("-> user {:?}", todo.user().get(&mut db).await?);
    }

    // Delete user
    let user = User::get_by_id(&mut db, &u2.id).await?;
    user.delete(&mut db).await?;
    assert!(User::get_by_id(&mut db, &u2.id).await.is_err());

    // Create a batch of users
    User::create_many()
        .item(User::create().email("foo@example.com").name("User Foo"))
        .item(User::create().email("bar@example.com").name("User Bar"))
        .exec(&mut db)
        .await?;

    // Lets create a new user. This time, we will batch create todos for the
    // user
    let mut user = User::create()
        .name("Ann Chovey")
        .email("ann.chovey@example.com")
        .todo(Todo::create().title("Make pizza"))
        .todo(Todo::create().title("Sleep"))
        .exec(&mut db)
        .await?;

    user.update()
        .todo(Todo::create().title("might delete later"))
        .exec(&mut db)
        .await?;

    // Get the last todo so we can unlink it
    let todos = user.todos().collect::<Vec<_>>(&mut db).await?;
    let len = todos.len();

    user.todos().remove(&mut db, todos.last().unwrap()).await?;

    assert_eq!(len - 1, user.todos().collect::<Vec<_>>(&mut db).await?.len());

    println!(">>> DONE <<<");

    Ok(())
}
