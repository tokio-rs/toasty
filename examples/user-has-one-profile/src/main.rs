use toasty::stmt::Id;

#[derive(Debug)]
#[toasty::model]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    name: String,

    #[has_one]
    profile: Option<Profile>,
}

#[derive(Debug)]
#[toasty::model]
struct Profile {
    #[key]
    #[auto]
    id: Id<Self>,

    #[belongs_to(key = user_id, references = id)]
    user: Option<User>,

    #[unique]
    user_id: Option<Id<User>>,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let db = toasty::Db::builder()
        .register::<User>()
        .register::<Profile>()
        .build(toasty_sqlite::Sqlite::in_memory())
        .await?;

    // For now, reset!s
    db.reset_db().await?;

    // Create a user without a profile
    let user = User::create().name("John Doe").exec(&db).await?;

    println!("user = {user:#?}");

    if let Some(profile) = user.profile().get(&db).await? {
        println!("profile: {:#?}", profile);
        println!("profile.user_id: {:#?}", profile.user_id);
    } else {
        println!("user has no profile");
    }

    Ok(())
}
