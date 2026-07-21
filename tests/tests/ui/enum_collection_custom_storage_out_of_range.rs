#[derive(toasty::Embed)]
enum Status {
    #[column(variant = 8388608)]
    Invalid,
}

#[derive(toasty::Model)]
struct Item {
    #[key]
    id: i64,
    #[column(type = int(3))]
    statuses: Vec<Status>,
}

fn main() {}
