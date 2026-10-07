//! Borrowed JSON setters: `&value` and `Json(&value)` are accepted wherever
//! the owned forms are, so a caller that still needs the value afterwards
//! doesn't have to clone it.
//!
//! The borrowed impls serialize through the same path as the owned ones, so
//! there is nothing driver-specific to check. What is worth checking is that
//! each setter entry point resolves the borrowed impl at all — a compile-time
//! property. One in-memory SQLite pass covers that and confirms the value
//! actually round-trips.

#![cfg(feature = "sqlite")]

use toasty::{Deferred, Json};

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    #[auto]
    id: uuid::Uuid,

    // set with `&value`
    #[column(type = text)]
    tags: Json<Vec<String>>,

    // set with `Json(&value)`
    #[column(type = text)]
    extra: Json<Vec<String>>,

    // nullable: takes `Json(&value)` for the same reason the owned form takes
    // `Json(value)` — the `Some(...)` is optional, the wrapper is not.
    #[column(type = text)]
    data: Option<Json<Vec<String>>>,

    // the borrowed setters resolve through `Deferred`'s expression target too
    #[column(type = text)]
    deferred: Deferred<Json<Vec<String>>>,
}

async fn setup() -> toasty::Db {
    let db = toasty::Db::builder()
        .models(toasty::models!(Item))
        .build(toasty_driver_sqlite::Sqlite::in_memory())
        .await
        .unwrap();

    db.push_schema().await.unwrap();
    db
}

#[tokio::test]
async fn borrowed_values_set_json_fields() {
    let mut db = setup().await;

    let one = vec!["rust".to_string(), "toasty".to_string()];
    let two = vec!["b".to_string()];

    // `create!`, then the create builder — both borrow the same value.
    let record = toasty::create!(Item {
        tags: &one,
        extra: Json(&one),
        data: Json(&one),
        deferred: &one,
    })
    .exec(&mut db)
    .await
    .unwrap();
    assert_stored(&mut db, &record.id, &one).await;
    assert_eq!(record.deferred.get().0, one);

    let mut record = Item::create()
        .tags(&one)
        .extra(Json(&one))
        .data(Json(&one))
        .deferred(Json(&one))
        .exec(&mut db)
        .await
        .unwrap();
    assert_stored(&mut db, &record.id, &one).await;
    assert_eq!(record.deferred.get().0, one);

    // Update builder, then `update!`.
    record
        .update()
        .tags(&two)
        .extra(Json(&two))
        .data(Json(&two))
        .deferred(&two)
        .exec(&mut db)
        .await
        .unwrap();
    assert_stored(&mut db, &record.id, &two).await;
    assert_eq!(record.deferred.get().0, two);

    toasty::update!(record {
        tags: &one,
        extra: Json(&one),
        data: Json(&one),
        deferred: Json(&one),
    })
    .exec(&mut db)
    .await
    .unwrap();
    assert_stored(&mut db, &record.id, &one).await;
    assert_eq!(record.deferred.get().0, one);

    // The borrowed values outlive every setter that took them.
    assert_eq!(one, vec!["rust".to_string(), "toasty".to_string()]);
    assert_eq!(two, vec!["b".to_string()]);
}

async fn assert_stored(db: &mut toasty::Db, id: &uuid::Uuid, expected: &[String]) {
    let read = Item::get_by_id(db, id).await.unwrap();

    assert_eq!(read.tags.0, expected);
    assert_eq!(read.extra.0, expected);
    assert_eq!(read.data.unwrap().0, expected);
}
