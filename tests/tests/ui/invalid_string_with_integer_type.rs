use toasty::stmt::Id;

#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    // Invalid: String with integer storage
    #[column(type = integer(4))]
    name: String,
}

fn main() {}
