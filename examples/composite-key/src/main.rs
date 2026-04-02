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
}

#[derive(Debug, toasty::Model)]
#[key(partition = user_id, local = id)]
struct Todo {
    #[auto]
    id: uuid::Uuid,

    title: String,

    order: i64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,

    user_id: uuid::Uuid,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let mut db = toasty::Db::builder()
        .models(toasty::models!(User, Todo))
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
        .exec(&mut db)
        .await?;

    println!("created user; name={:?}; email={:?}", user.name, user.email);

    for (i, title) in ["finish toasty", "retire", "play golf"].iter().enumerate() {
        let todo = user
            .todos()
            .create()
            .title(*title)
            .order(i as i64)
            .exec(&mut db)
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

    let todos = user
        .todos()
        .query(Todo::fields().order().eq(1))
        .exec(&mut db)
        .await?;

    for todo in todos {
        println!("TODO = {todo:#?}");
    }

    Ok(())
}
