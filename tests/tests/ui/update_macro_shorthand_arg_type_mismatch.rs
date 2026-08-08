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

    // `name` is a String field; the type error should point at `false`,
    // even though the macro hoists the shorthand argument out of the
    // builder chain.
    let _ = toasty::update!(user { name.set(false) });
}
