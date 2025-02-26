#[toasty_macros::model]
struct User {
    #[key]
    #[auto]
    id: i32,

    name: String,
}

// #[toasty_macros::model]
// struct Todo {
//     id: i32,
//     name: String,
// }

#[tokio::main]
async fn main() -> toasty::Result<()> {
    Ok(())
}
