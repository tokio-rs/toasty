/// Example showing database-side auto-increment for integer IDs
///
/// Note: SQLite requires INTEGER type for AUTOINCREMENT, so use i32 not i64
/// For other databases like PostgreSQL and MySQL, i64 would work fine with SERIAL/AUTO_INCREMENT
#[derive(Debug, toasty::Model)]
pub struct Post {
    #[key]
    #[auto]
    pub id: i32,

    pub title: String,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let mut db = toasty::Db::builder()
        .register::<Post>()
        .connect("sqlite::memory:")
        .await?;

    db.push_schema().await?;

    println!("==> Inserting posts with auto-increment IDs...");

    let post1 = Post::create().title("First Post").exec(&mut db).await?;

    let post2 = Post::create().title("Second Post").exec(&mut db).await?;

    println!("Post 1 ID: {} (database-generated)", post1.id);
    println!("Post 2 ID: {} (database-generated)", post2.id);

    println!("\n>>> Success! <<<");
    Ok(())
}
