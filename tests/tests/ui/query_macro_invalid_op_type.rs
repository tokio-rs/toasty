#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i64,
    name: String,
}

fn main() {
    // Comparing a string field with an integer should produce a type error
    // pointing at the comparison expression
    let _ = toasty::query!(User filter .name == 42);
}
