#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    id: uuid::Uuid,
    values: Vec<Option<String>>,
}

fn main() {}
