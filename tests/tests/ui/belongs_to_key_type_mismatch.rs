// A `belongs_to` whose `key` field type does not match the target model's
// primary key type must be rejected. Here `Pet::owner_id` is a `uuid::Uuid` but
// `Human::id` is a `u32`.

#[derive(Debug, toasty::Model)]
struct Human {
    #[key]
    #[auto]
    id: u32,
    name: String,
}

#[derive(Debug, toasty::Model)]
struct Pet {
    #[key]
    #[auto]
    id: uuid::Uuid,
    owner_id: uuid::Uuid,
    #[belongs_to(key = owner_id, references = id)]
    owner: toasty::Deferred<Human>,
}

fn main() {}
