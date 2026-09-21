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
