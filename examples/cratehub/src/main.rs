use toasty::stmt::Id;

#[derive(Debug)]
#[toasty::model(table = user_and_packages)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    name: String,

    #[unique]
    email: String,

    #[has_many]
    packages: [Package],
}

#[derive(Debug)]
#[toasty::model(table = user_and_packages)]
#[key(partition = user_id, local = id)]
struct Package {
    #[belongs_to(key = user_id, references = id)]
    user: User,

    user_id: Id<User>,

    #[auto]
    id: Id<Self>,

    name: String,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let db = toasty::Db::builder()
        .register::<User>()
        .register::<Package>()
        .build(toasty_sqlite::Sqlite::in_memory())
        .await?;

    // For now, reset!s
    db.reset_db().await?;

    println!("==> let u1 = User::create()");
    let u1 = User::create()
        .name("John Doe")
        .email("john@example.com")
        .exec(&db)
        .await?;

    println!(" -> u1 = {u1:#?}");

    println!("==> let u2 = User::create()");
    let u2 = User::create()
        .name("Jane doe")
        .email("jane@example.com")
        .exec(&db)
        .await?;
    println!(" -> u2 = {u2:#?}");

    let p1 = u1.packages().create().name("tokio").exec(&db).await?;

    println!("==> Package::find_by_user_and_id(&u1, &p1.id)");
    let package = Package::get_by_user_id_and_id(&db, &u1.id, &p1.id).await?;

    println!("{package:#?}");

    println!("==> u1.packages().all(&db)");
    let packages = u1.packages().all(&db).await?.collect::<Vec<_>>().await;
    println!("packages = {packages:#?}");

    Ok(())
}
