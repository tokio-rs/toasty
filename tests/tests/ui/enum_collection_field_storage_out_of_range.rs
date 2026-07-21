#[derive(toasty::Embed)]
enum Status {
    #[column(variant = 256)]
    Invalid,
}

#[derive(toasty::Model)]
struct Item {
    #[key]
    id: i64,
    #[column(type = u8)]
    statuses: Vec<Status>,
}

fn main() {}
