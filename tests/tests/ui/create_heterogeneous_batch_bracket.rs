#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i32,
    name: String,
}

#[derive(toasty::Model)]
struct Article {
    #[key]
    #[auto]
    id: i32,
    title: String,
}

fn main() {
    // Bracket syntax is not valid for heterogeneous batch creation;
    // use tuple syntax `(User { ... }, Article { ... })` instead.
    let _ = toasty::create!([User { name: "Carl" }, Article { title: "Hello" }]);
}
