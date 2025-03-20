use toasty::stmt::Id;

#[toasty::model]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    #[has_one]
    profile: Option<Profile>,
}

#[toasty::model]
struct Profile {
    #[key]
    #[auto]
    id: Id<Self>,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let _ = <Option<Profile> as toasty::relation::Relation2>::Model::FIELDS.user;

    Ok(())
}
