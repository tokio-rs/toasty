use toasty::stmt::Id;

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    name: String,

    #[has_one]
    profile: toasty::HasOne<Option<Profile>>,
}

#[derive(Debug, toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: Id<Self>,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<Option<User>>,

    #[unique]
    user_id: Option<Id<User>>,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let db = toasty::Db::builder()
        .register::<User>()
        .register::<Profile>()
        .connect(
            std::env::var("TOASTY_CONNECTION_URL")
                .as_deref()
                .unwrap_or("sqlite::memory:"),
        )
        .await?;

    // For now, reset!s
    db.push_schema().await?;

    // Create a user without a profile
    let user = User::create().name("John Doe").exec(&db).await?;

    println!("created user; name={:?}", user.name);

    if let Some(profile) = user.profile().get(&db).await? {
        println!("profile: {profile:#?}");
        println!("profile.user_id: {:#?}", profile.user_id);
    } else {
        println!("user has no profile");
    }

    Ok(())
}
