use toasty::stmt::Id;

#[derive(Debug)]
#[toasty::model]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    #[has_one]
    profile: Option<Profile>,
}

#[derive(Debug)]
#[toasty::model]
struct Profile {
    #[key]
    #[auto]
    id: Id<Self>,

    #[unique]
    user_id: Id<User>,

    #[belongs_to(key = user_id, references = id)]
    user: User,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    Ok(())
}
