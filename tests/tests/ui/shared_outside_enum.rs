// `#[shared]` declares a logical field shared across enum variants; it has no
// meaning on a root model or embedded struct field.

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: String,
    #[shared(name)]
    name: String,
}

fn main() {}
