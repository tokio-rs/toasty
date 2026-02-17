use toasty::stmt::Id;

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    name: String,

    #[unique]
    email: String,

    #[has_many]
    todos: toasty::HasMany<Todo>,
}

#[derive(Debug, toasty::Model)]
#[key(partition = user_id, local = id)]
struct Todo {
    #[auto]
    id: Id<Self>,

    title: String,

    order: i64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,

    user_id: Id<User>,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let db = toasty::Db::builder()
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

    let user = User::create()
        .name("John Doe")
        .email("john@example.com")
        .exec(&db)
        .await?;

    println!("created user; name={:?}; email={:?}", user.name, user.email);

    for (i, title) in ["finish toasty", "retire", "play golf"].iter().enumerate() {
        let todo = user
            .todos()
            .create()
            .title(*title)
            .order(i as i64)
            .exec(&db)
            .await?;

        println!(
            "created todo; title={:?}; order={:?}",
            todo.title, todo.order
        );
    }

    // Query a user's todos
    println!("====================");
    println!("--- QUERY ---");
    println!("====================");

    let mut todos = user
        .todos()
        .query(Todo::fields().order().eq(1))
        .all(&db)
        .await?;

    while let Some(todo) = todos.next().await {
        let todo = todo?;
        println!("TODO = {todo:#?}");
    }

    Ok(())
}
