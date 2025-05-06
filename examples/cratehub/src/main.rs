use toasty::stmt::Id;

#[derive(Debug, toasty::Model)]
#[table = "user_and_packages"]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    name: String,

    #[unique]
    email: String,

    #[has_many]
    packages: toasty::HasMany<Package>,
}

#[derive(Debug, toasty::Model)]
#[table = "user_and_packages"]
#[key(partition = user_id, local = id)]
struct Package {
    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,

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
        .connect(
            std::env::var("TOASTY_CONNECTION_URL")
                .as_deref()
                .unwrap_or("sqlite::memory:"),
        )
        .await?;

    // For now, reset!s
    db.reset_db().await?;

    let u1 = User::create()
        .name("John Doe")
        .email("john@example.com")
        .exec(&db)
        .await?;
    println!("created user; name={:?}; email={:?}", u1.name, u1.email);

    let u2 = User::create()
        .name("Jane doe")
        .email("jane@example.com")
        .exec(&db)
        .await?;
    println!("created user; name={:?}; email={:?}", u2.name, u2.email);

    let p1 = u1.packages().create().name("tokio").exec(&db).await?;

    println!("==> Package::find_by_user_and_id(&u1, &p1.id)");
    let package = Package::get_by_user_id_and_id(&db, &u1.id, &p1.id).await?;

    println!("package; name={:?}", package.name);

    println!("==> u1.packages().all(&db)");
    let packages = u1.packages().all(&db).await?.collect::<Vec<_>>().await;
    println!("packages = {packages:#?}");

    Ok(())
}
