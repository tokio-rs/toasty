// A `belongs_to` inside an embedded enum variant whose `key` field type does
// not match the target model's primary key type must be rejected. Here
// `Owner::Human::id` is a `uuid::Uuid` but `Human::id` is a `u32`.

#[derive(Debug, toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: u32,
    name: String,
}

#[derive(Debug, toasty::Model)]
struct Animal {
    #[key]
    #[auto]
    id: uuid::Uuid,
    name: String,
    color: String,
}

#[derive(Debug, toasty::Embed)]
enum Owner {
    Human {
        #[shared(id)]
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        human: toasty::Deferred<Human>,
    },
    Animal {
        #[shared(id)]
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        animal: toasty::Deferred<Animal>,
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
