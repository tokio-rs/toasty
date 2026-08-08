// A relation stored in an embedded type carries no storage; its record slot
// always decodes from `Null`, which only `toasty::Deferred<..>` represents
// (as the unloaded state). A bare model type could never load.

#[derive(Debug, toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: uuid::Uuid,
}

#[derive(Debug, toasty::Embed)]
enum Owner {
    Human {
        #[index]
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        human: Human,
    },
}

fn main() {}
