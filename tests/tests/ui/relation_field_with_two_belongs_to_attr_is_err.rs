use toasty::stmt::Id;

#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,
}

#[derive(toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: Id<Self>,

    #[index]
    user_id: Id<User>,

    #[belongs_to(key = user_id, references = id)]
    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,
}

fn main() {}