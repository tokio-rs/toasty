// A newtype embed used as an `#[auto]` field must opt into the proxy via
// struct-level `#[auto]`. Without it the inner type's `Auto` impl never
// reaches the field and the user sees a missing-impl error.

#[derive(Debug, toasty::Embed)]
struct UserId(uuid::Uuid);

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: UserId,
    name: String,
}

fn main() {}
