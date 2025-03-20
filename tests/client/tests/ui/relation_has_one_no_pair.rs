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

fn main() {}