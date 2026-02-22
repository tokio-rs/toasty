#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,
}

#[derive(toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[index]
    user_id: uuid::Uuid,

    #[belongs_to(key = user_id, references = id)]
    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,
}

fn main() {}