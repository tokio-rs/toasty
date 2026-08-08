#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    profile: toasty::Deferred<Option<Profile>>,
}

#[derive(toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[unique]
    user_id: uuid::Uuid,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::Deferred<User>,
}

fn main() {}