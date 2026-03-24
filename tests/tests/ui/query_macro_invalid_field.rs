#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i64,
    name: String,
}

fn main() {
    // `.nonexistent` should point to the bad field name
    let _ = toasty::query!(User filter .nonexistent == "Alice");
}
