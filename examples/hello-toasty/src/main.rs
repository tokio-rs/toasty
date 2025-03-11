use toasty::stmt::Id;

#[derive(Debug)]
#[toasty::model]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    name: String,

    #[unique]
    email: String,

    #[has_many]
    todos: [Todo],

    moto: Option<String>,
}

#[derive(Debug)]
#[toasty::model]
struct Todo {
    #[key]
    #[auto]
    id: Id<Self>,

    #[index]
    user_id: Id<User>,

    #[belongs_to(key = user_id, references = id)]
    user: User,

    title: String,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let mut builder = toasty::Db::builder();
    builder.register::<User>().register::<Todo>();

    cfg_if::cfg_if! {
        if #[cfg(feature = "sqlite")] {
            let db = builder.build(toasty_sqlite::Sqlite::in_memory()).await?;
        } else if #[cfg(feature = "postgresql")] {
                let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                        panic!(
                            "`DATABASE_URL` environment variable is required when using \
                            the `postgresql` feature (e.g., \
                            `DATABASE_URL=postgresql://postgres@localhost/toasty`)"
                        );
                    }
                );

            let db = builder.build(toasty_pgsql::PostgreSQL::connect(&url, postgres::NoTls)).await?;
        } else {
            drop(builder);
            #[allow(unused_variables)]
            let db: toasty::Db;
            panic!("you must run this example with a database-related feature enabled (e.g., `--features sqlite`)")
        }
    };

    #[allow(unreachable_code)] // TODO: fix
    // For now, reset!s
    db.reset_db().await?;

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

    // Find by ID
    println!("==> let user = User::find_by_id(&u1.id)");
    let user = User::get_by_id(&db, &u1.id).await?;
    println!("USER = {user:#?}");

    // Find by email!
    println!("==> let user = User::find_by_email(&u1.email)");
    let mut user = User::get_by_email(&db, &u1.email).await?;
    println!("USER = {user:#?}");

    assert!(User::create()
        .name("John Dos")
        .email("john@example.com")
        .exec(&db)
        .await
        .is_err());

    user.update().name("Foo bar").exec(&db).await?;
    assert_eq!(user.name, "Foo bar");
    assert_eq!(User::get_by_id(&db, &user.id).await?.name, user.name);

    // Load the user again
    let user = User::get_by_id(&db, &u1.id).await?;
    println!("  reloaded, notice change to the user's name -> {user:#?}");

    println!(" ~~~~~~~~~~~ CREATE TODOs ~~~~~~~~~~~~");

    let todo = u2.todos().create().title("finish toasty").exec(&db).await?;

    println!("CREATED = {todo:#?}");

    let mut todos = u2.todos().all(&db).await?;

    while let Some(todo) = todos.next().await {
        let todo = todo?;
        println!("TODO = {todo:#?}");
        println!("-> user {:?}", todo.user().get(&db).await?);
    }

    // Delete user
    let user = User::get_by_id(&db, &u2.id).await?;
    user.delete(&db).await?;
    assert!(User::get_by_id(&db, &u2.id).await.is_err());

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

    user.todos().remove(&db, todos.last().unwrap()).await?;

    assert_eq!(len - 1, user.todos().collect::<Vec<_>>(&db).await?.len());

    println!(">>> DONE <<<");

    Ok(())
}
