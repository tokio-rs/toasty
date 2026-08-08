// A bare `#[belongs_to]` infers its foreign key field from the relation field
// name (`user` -> `user_id`). When that field is missing, the macro must reject
// it at compile time rather than deferring to a runtime schema error.

#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,
}

#[derive(toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[belongs_to]
    user: toasty::Deferred<User>,
}

fn main() {}
