#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i64,
    name: String,
}

fn main() {
    // `42_i64` does not implement `Scope`, the error should point at `42_i64`
    let _ = toasty::create!(in 42_i64 { name: "Carl" });
}
