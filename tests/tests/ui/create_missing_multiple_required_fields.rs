#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i64,

    name: String,
    email: String,
}

fn main() {
    // Missing both `name` and `email`
    let _ = toasty::create!(User {});
}
