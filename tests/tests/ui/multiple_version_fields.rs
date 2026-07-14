#[derive(toasty::Model)]
struct Document {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[version]
    version: u64,

    #[version]
    other_version: u64,
}

fn main() {}
