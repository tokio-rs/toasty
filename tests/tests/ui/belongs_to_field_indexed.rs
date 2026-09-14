// A relation field maps to no column, so there is nothing to index; the
// index belongs on the foreign key field(s). Same rule as embedded types,
// applied to a root model.

#[derive(Debug, toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: uuid::Uuid,
}

#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: uuid::Uuid,

    author_id: uuid::Uuid,
    #[index]
    #[belongs_to(key = author_id, references = id)]
    author: toasty::BelongsTo<Human>,
}

fn main() {}
