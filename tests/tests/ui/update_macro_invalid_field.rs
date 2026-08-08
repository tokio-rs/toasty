#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i64,
    name: String,
}

fn main() {
    let mut user = User {
        id: 1,
        name: "Alice".to_string(),
    };

    // `nonexistent` is not a field on User; the error should point at `nonexistent`
    let _ = toasty::update!(user { nonexistent: "value" });
}
