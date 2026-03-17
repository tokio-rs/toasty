#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i64,

    name: String,
    email: String,
    bio: Option<String>,
}

fn main() {
    // Missing `email` — should produce a compile error
    let _ = toasty::create!(User { name: "Carl" });
}
