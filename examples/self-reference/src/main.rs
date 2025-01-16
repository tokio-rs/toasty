mod db;

use std::path::PathBuf;
use toasty::Db;
use toasty_sqlite::Sqlite;

#[tokio::main]
async fn main() {
    let schema_file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema.toasty");

    let schema = toasty::schema::from_file(schema_file).unwrap();

    // NOTE enable this to see the enstire structure in STDOUT
    // println!("{schema:#?}");

    // Use the in-memory sqlite driver
    let driver = Sqlite::in_memory();

    let db = Db::new(schema, driver).await;
    // For now, reset!s
    db.reset_db().await.unwrap();

    let p1 = db::Person::create()
        .name("Person 1")
        .exec(&db)
        .await
        .unwrap();

    let p2 = db::Person::create()
        .name("Person 2")
        .parent_id(p1.id.clone())
        .exec(&db)
        .await
        .unwrap();

    let parent = p2.parent().find(&db).await.unwrap();
    assert_eq!(Some(p1.id.clone()), parent.map(|p| p.id));

    println!("P1: {:#?}", p1);
    println!("P2: {:#?}", p2);
}
