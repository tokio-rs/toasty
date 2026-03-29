#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i64,
    name: String,
}

fn main() {
    // `nonexistent` is not a field on User; the error should point at `nonexistent`
    let _ = toasty::create!(User { nonexistent: "value" });
}
