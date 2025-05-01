use toasty::stmt::Id;

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    #[has_one]
    profile: toasty::HasOne<Option<Profile>>,
}

#[derive(Debug, toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: Id<Self>,

    #[unique]
    user_id: Id<User>,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    Ok(())
}
