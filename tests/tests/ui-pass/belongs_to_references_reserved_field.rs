#![allow(dead_code)]

// A `belongs_to` may reference a target field whose name collides with a
// built-in fields-struct method (`filter`, `eq`, ...). Such fields have no
// public accessor, so the generated key-type checks must reach them another
// way. Covers both a root model and an embedded enum variant.

#[derive(Debug, toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: uuid::Uuid,
    filter: String,
}

#[derive(Debug, toasty::Model)]
struct Pet {
    #[key]
    #[auto]
    id: uuid::Uuid,
    owner_filter: String,
    #[belongs_to(key = owner_filter, references = filter)]
    owner: toasty::Deferred<Human>,
}

#[derive(Debug, toasty::Embed)]
enum Owner {
    #[column(variant = 1)]
    Human {
        filter: String,
        #[belongs_to(key = filter, references = filter)]
        human: toasty::Deferred<Human>,
    },
}

#[derive(Debug, toasty::Model)]
struct Object {
    #[key]
    #[auto]
    id: uuid::Uuid,
    owner: Owner,
}

fn main() {}
