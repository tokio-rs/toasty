#[derive(toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: i32,
    username: String,
}

fn main() {
    let _ = User::filter_by_id(1).include();
}
