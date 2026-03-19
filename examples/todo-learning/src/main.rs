use example_todo_learning::User;

#[tokio::main]
async fn main() -> toasty::Result<()> {
    // Initialize tracing to see the query engine pipeline
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Create database connection
    let mut db = toasty::Db::builder()
        .register::<User>()
        .connect("sqlite::memory:")
        .await?;

    // Push the schema to the database (creates tables)
    db.push_schema().await?;

    println!("==> Inserting a user...");

    // Insert operation: creates a User record
    let users = User::create_many()
        .item(User::create().name("Alice").email("alice@example.com"))
        .item(User::create().name("Bob").email("bob@example.com"))
        .exec(&mut db)
        .await?;
    let alice = &users[0];
    println!("Inserted user with ID: {}", alice.id);
    println!("  name: {}", alice.name);
    println!("  email: {}", alice.email);

    println!("\n==> Querying user by primary key...");

    // Query operation: fetch by primary key
    let fetched_user = User::get_by_id(&mut db, &alice.id).await?;

    println!("Fetched user with ID: {}", fetched_user.id);
    println!("  name: {}", fetched_user.name);
    println!("  email: {}", fetched_user.email);

    println!("\n>>> Success! <<<");

    Ok(())
}
