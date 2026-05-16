// The legacy flat form `key = a, references = b, key = c, references = d` is
// ambiguous for composite foreign keys. The macro must reject it and tell the
// user to switch to the bracketed list form.

#[derive(toasty::Model)]
#[key(id, revision)]
struct Parent {
    id: uuid::Uuid,
    revision: i64,
}

#[derive(toasty::Model)]
struct Child {
    #[key]
    #[auto]
    id: uuid::Uuid,

    parent_id: uuid::Uuid,
    parent_revision: i64,

    #[belongs_to(
        key = parent_id, references = id,
        key = parent_revision, references = revision,
    )]
    parent: toasty::BelongsTo<Parent>,
}

fn main() {}
