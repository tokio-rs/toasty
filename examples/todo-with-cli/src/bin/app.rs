use example_todo_with_cli::{Todo, User, create_db};

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let db = create_db().await?;

    println!("==> Creating users...");
    let user1 = User::create()
        .name("Alice")
        .email("alice@example.com")
        .exec(&db)
        .await?;

    let user2 = User::create()
        .name("Bob")
        .email("bob@example.com")
        .exec(&db)
        .await?;

    println!("Created users: {} and {}", user1.name, user2.name);

    println!("\n==> Creating todos...");
    let todo1 = user1
        .todos()
        .create()
        .title("Learn Rust")
        .completed(false)
        .exec(&db)
        .await?;

    let todo2 = user1
        .todos()
        .create()
        .title("Build a web app")
        .completed(false)
        .exec(&db)
        .await?;

    let _todo3 = user2
        .todos()
        .create()
        .title("Write documentation")
        .completed(true)
        .exec(&db)
        .await?;

    println!("Created {} todos", 3);

    println!("\n==> Listing all users and their todos...");
    let users = User::all().collect::<Vec<_>>(&db).await?;

    for user in users {
        println!("\nUser: {} ({})", user.name, user.email);

        let mut todos = user.todos().all(&db).await?;
        while let Some(todo) = todos.next().await {
            let todo = todo?;
            let status = if todo.completed { "✓" } else { " " };
            println!("  [{}] {}", status, todo.title);
        }
    }

    println!("\n==> Updating a todo...");
    let mut todo = Todo::get_by_id(&db, &todo1.id).await?;
    todo.update().completed(true).exec(&db).await?;
    println!("Marked '{}' as completed", todo.title);

    println!("\n==> Deleting a todo...");
    let todo = Todo::get_by_id(&db, &todo2.id).await?;
    println!("Deleting '{}'", todo.title);
    todo.delete(&db).await?;

    println!("\n==> Final count...");
    let todos = Todo::all().collect::<Vec<_>>(&db).await?;
    println!("Total todos remaining: {}", todos.len());

    println!("\n>>> Application completed successfully! <<<");

    Ok(())
}
