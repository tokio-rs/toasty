#[derive(Debug)]
#[toasty_macros::model]
struct User {
    #[key]
    #[auto]
    id: toasty::stmt::Id<User>,

    name: String,
}

// #[toasty_macros::model]
// struct Todo {
//     id: i32,
//     name: String,
// }

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let schema = toasty::schema::from_macro(&[User::schema()])?;
    println!("{schema:#?}");

    let driver = toasty_sqlite::Sqlite::in_memory();
    let db = toasty::Db::new(schema, driver).await?;

    // For now, reset!s
    db.reset_db().await?;

    let user = User::create().name("John Doe").exec(&db).await?;

    println!("{user:#?}");

    Ok(())
}
