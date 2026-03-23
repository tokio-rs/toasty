#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i32,
    name: String,
    email: String,
}

fn main() {
    // v1 syntax with comma separator is no longer valid
    let _ = toasty::create!(User, { name: "Carl", email: "carl@example.com" });
}
