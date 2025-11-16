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
    posts: toasty::HasMany<Post>,
}

#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: Id<Self>,
    #[index]
    user_id: Id<User>,
    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,
    title: String,
    content: String,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    println!("MongoDB Toasty Example");
    let db = toasty::Db::builder()
        .register::<User>()
        .register::<Post>()
        .connect(
            std::env::var("TOASTY_CONNECTION_URL")
                .as_deref()
                .unwrap_or("mongodb://localhost:27017/toasty_example"),
        )
        .await?;
    
    db.reset_db().await?;
    
    let user = User::create()
        .name("Alice")
        .email("alice@example.com")
        .exec(&db)
        .await?;
    
    println!("Created user: {:?}", user);
    Ok(())
}
