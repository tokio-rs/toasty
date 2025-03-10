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
}

#[derive(Debug)]
#[toasty::model]
#[key(partition = user_id, local = id)]
struct Todo {
    #[auto]
    id: Id<Self>,

    title: String,

    order: i64,

    #[belongs_to(key = user_id, references = id)]
    user: User,

    user_id: Id<User>,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let db = toasty::Db::builder()
        .register::<User>()
        .register::<Todo>()
        .build(toasty_sqlite::Sqlite::in_memory())
        .await?;

    // For now, reset!s
    db.reset_db().await?;

    println!("==> let user = User::create()");
    let user = User::create()
        .name("John Doe")
        .email("john@example.com")
        .exec(&db)
        .await?;

    println!(" ~~~~~~~~~~~ CREATE TODOs ~~~~~~~~~~~~");

    for (i, title) in ["finish toasty", "retire", "play golf"].iter().enumerate() {
        let todo = user
            .todos()
            .create()
            .title(*title)
            .order(i as i64)
            .exec(&db)
            .await?;

        println!("CREATED = {todo:#?}");
    }

    // Query a user's todos
    println!("====================");
    println!("--- QUERY ---");
    println!("====================");

    let mut todos = user
        .todos()
        .query(Todo::FIELDS.order.eq(1))
        .all(&db)
        .await?;

    while let Some(todo) = todos.next().await {
        let todo = todo?;
        println!("TODO = {todo:#?}");
    }

    println!(">>> DONE <<<");

    Ok(())
}
