use toasty::stmt::Id;

#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    profile: toasty::HasOne<Option<Profile>>,
}

#[derive(toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: Id<Self>,

    #[unique]
    user_id: Id<User>,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,
}

fn main() {}