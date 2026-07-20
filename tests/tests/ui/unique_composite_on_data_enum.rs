// Same restriction through a model-level composite `#[unique(...)]`: naming a
// data-carrying embedded enum among the unique columns is a compile error.
// This is the shape from issue #973, but with a data-carrying enum instead of a
// unit enum.

#[derive(Debug, toasty::Embed)]
enum Contact {
    #[column(variant = 1)]
    Email { address: String },
    #[column(variant = 2)]
    Phone { number: String },
}

#[derive(Debug, toasty::Model)]
#[unique(contact, slug)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    contact: Contact,
    slug: String,
}

fn main() {}
