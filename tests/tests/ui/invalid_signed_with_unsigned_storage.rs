use toasty::stmt::Id;

#[derive(toasty::Model)]
struct Product {
    #[key]
    #[auto]
    id: Id<Self>,

    // Invalid: i32 with unsigned storage
    #[column(type = unsignedinteger(4))]
    price: i32,
}

fn main() {}
