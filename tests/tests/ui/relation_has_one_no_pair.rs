use toasty::stmt::Id;

#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    #[has_one]
    profile: toasty::HasOne<Option<Profile>>,
}

#[derive(toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: Id<Self>,
}

fn main() {}