#[derive(toasty::Embed)]
#[column(type = u8)]
enum Status {
    #[column(variant = 256)]
    Invalid,
}

fn main() {}
