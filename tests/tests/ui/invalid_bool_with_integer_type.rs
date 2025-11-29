use toasty::stmt::Id;

#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    // Invalid: bool with integer storage
    #[column(type = integer(4))]
    is_active: bool,
}

fn main() {}
