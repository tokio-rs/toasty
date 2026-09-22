// `#[shared]` merges variant columns, but a relation field has no column to
// merge; sharing is declared on the foreign key field that owns the storage.

#[derive(Debug, toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: uuid::Uuid,
}

#[derive(Debug, toasty::Embed)]
enum Owner {
    Human {
        #[shared(id)]
        id: uuid::Uuid,
        #[shared(rel)]
        #[belongs_to(key = id)]
        human: toasty::Deferred<Human>,
    },
}

fn main() {}
