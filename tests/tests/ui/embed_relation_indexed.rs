// A relation field maps to no column, so there is nothing to index; the
// index belongs on the foreign key field(s).

#[derive(Debug, toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: uuid::Uuid,
}

#[derive(Debug, toasty::Embed)]
struct Attribution {
    author_id: uuid::Uuid,
    #[index]
    #[belongs_to(key = author_id)]
    author: toasty::Deferred<Human>,
}

fn main() {}
