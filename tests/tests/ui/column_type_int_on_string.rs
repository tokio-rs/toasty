// Storing a `String` field as an integer column makes no sense — the
// `CompatibleWith` obligation must reject it.

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i64,

    #[column(type = i64)]
    name: String,
}

fn main() {}
