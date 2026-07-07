// A data-carrying embedded enum maps to a discriminant column plus per-variant
// data columns, so it has no single index column. Indexing one must be rejected
// at compile time, not panic when the schema is built. A unit (data-less) enum
// is fine — only data variants are the problem.

#[derive(Debug, toasty::Embed)]
enum Contact {
    #[column(variant = 1)]
    Email { address: String },
    #[column(variant = 2)]
    Phone { number: String },
}

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[index]
    contact: Contact,
}

fn main() {}
