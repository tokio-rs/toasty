// A relation field maps to no column, so it cannot serve as a unique
// column; a model-level #[unique(...)] must name the foreign key field(s)
// instead.

#[derive(Debug, toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: uuid::Uuid,
}

#[derive(Debug, toasty::Model)]
#[unique(author, slug)]
struct Post {
    #[key]
    #[auto]
    id: uuid::Uuid,

    author_id: uuid::Uuid,
    #[belongs_to(key = author_id, references = id)]
    author: toasty::BelongsTo<Human>,

    slug: String,
}

fn main() {}
