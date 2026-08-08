#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[has_one]
    profile: toasty::Deferred<Option<Profile>>,
}

#[derive(toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: uuid::Uuid,
}

fn main() {}