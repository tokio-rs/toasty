use toasty::stmt::Id;

#[derive(toasty::Model)]
struct Counter {
    #[key]
    #[auto]
    id: Id<Self>,

    // Invalid: i32 with undersized integer storage (i32 needs at least 4 bytes)
    #[column(type = integer(1))]
    count: i32,
}

fn main() {}