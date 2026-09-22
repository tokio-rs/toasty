// A relation field maps to no column, so there is nothing to index; an
// enum-level #[index(...)] must name the foreign key field(s) instead.

#[derive(Debug, toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: uuid::Uuid,
}

#[derive(Debug, toasty::Embed)]
#[index(human::human)]
enum Owner {
    Human {
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        human: toasty::Deferred<Human>,
    },
}

fn main() {}
