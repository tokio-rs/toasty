#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i64,
    name: String,
}

fn main() {
    // `name` is required (non-nullable, no default) — omitting it should
    // produce a compile-time error naming the missing field.
    let _ = toasty::create!(User {});
}
