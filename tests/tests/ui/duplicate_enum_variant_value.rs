#[derive(toasty::Embed)]
enum Status {
    #[column(variant = 1)]
    Active,
    #[column(variant = 1)]
    Inactive,
}

fn main() {}
