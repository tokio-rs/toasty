// `#[document]` requires a document-capable type: a `#[derive(Embed)]`
// struct or a `Vec` of them. A unit enum embed and a scalar collection must
// be rejected at compile time, not silently stored as ordinary columns.

#[derive(Debug, toasty::Embed)]
enum Role {
    Admin,
    Member,
}

#[derive(Debug, toasty::Model)]
struct Account {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[document]
    role: Role,

    #[document]
    tags: Vec<String>,
}

fn main() {}
