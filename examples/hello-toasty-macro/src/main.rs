#[toasty_macros::model]
struct User {
    #[key]
    // #[auto]
    id: i64,

    name: String,
}

// #[toasty_macros::model]
// struct Todo {
//     id: i32,
//     name: String,
// }

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let schema = toasty::schema::from_macro(&[User::schema()]);

    println!("schema={schema:#?}");

    Ok(())
}
