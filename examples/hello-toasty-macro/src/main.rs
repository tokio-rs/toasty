#[derive(Debug)]
#[toasty_macros::model]
struct User {
    #[key]
    #[auto]
    id: toasty::stmt::Id<User>,

    name: String,

    #[unique]
    email: String,
}

#[derive(Debug)]
#[toasty_macros::model]
struct Todo {
    #[key]
    #[auto]
    id: toasty::stmt::Id<Todo>,
    name: String,

    #[index]
    user_id: toasty::stmt::Id<User>,

    #[relation(key = user_id, references = id)]
    user: User,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let schema = toasty::schema::from_macro(&[User::schema(), Todo::schema()])?;
    println!("{schema:#?}");

    let driver = toasty_sqlite::Sqlite::in_memory();
    let db = toasty::Db::new(schema, driver).await?;

    // For now, reset!s
    db.reset_db().await?;

    let user = User::create()
        .name("John Doe")
        .email("john@example.com")
        .exec(&db)
        .await?;
    println!("{user:#?}");

    let user = User::get_by_email(&db, "john@example.com").await.unwrap();
    println!("{user:#?}");

    let todo = Todo::create()
        .user(&user)
        .name("Buy milk")
        .exec(&db)
        .await?;
    println!("{todo:#?}");

    Ok(())
}
