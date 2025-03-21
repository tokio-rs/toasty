use toasty::stmt::Id;

#[toasty::model]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    profile: Option<Profile>,
}

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

fn main() {}