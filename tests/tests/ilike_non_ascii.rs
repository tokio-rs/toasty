//! Regression coverage for case-insensitive `ilike` matching outside the ASCII
//! range on SQLite — https://github.com/tokio-rs/toasty/issues/802.
//!
//! `.ilike()` asks for case-insensitive matching. On PostgreSQL this lowers to
//! `ILIKE`, which case-folds Unicode correctly. On SQLite the serializer emits
//! plain `LIKE`, which only case-folds ASCII — so `.ilike("CAFÉ")` silently
//! fails to match `"café"`. The user gets a wrong (case-sensitive) result with
//! no error.
//!
//! The ASCII test below passes today and guards against regressing the case
//! that does work. The non-ASCII test is `#[ignore]`'d: it asserts the behavior
//! the fix should produce, mirroring the serializer-level placeholders added in
//! #922 (`crates/toasty-sql/tests/serialize_expressions.rs`). Drop the
//! `#[ignore]` once the serializer case-folds non-ASCII on SQLite.

#![cfg(feature = "sqlite")]

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    id: i64,
    name: String,
}

async fn db() -> toasty::Db {
    let db = toasty::Db::builder()
        .models(toasty::models!(Item))
        .build(toasty_driver_sqlite::Sqlite::in_memory())
        .await
        .unwrap();
    db.push_schema().await.unwrap();
    db
}

/// `ilike` is case-insensitive for ASCII on SQLite — this works today.
#[tokio::test]
async fn ilike_ascii_is_case_insensitive() {
    let mut db = db().await;
    toasty::create!(Item { id: 1_i64, name: "Bistro" })
        .exec(&mut db)
        .await
        .unwrap();

    let items: Vec<Item> = Item::filter(Item::fields().name().ilike("BISTRO".to_string()))
        .exec(&mut db)
        .await
        .unwrap();

    assert_eq!(items.len(), 1, "ASCII ilike should ignore case");
}

/// `ilike` should also be case-insensitive for non-ASCII characters. On SQLite
/// it currently is not, because the serializer emits plain `LIKE`.
#[ignore = "non-ASCII ilike is case-sensitive on SQLite — see #802"]
#[tokio::test]
async fn ilike_non_ascii_is_case_insensitive() {
    let mut db = db().await;
    toasty::create!(Item { id: 1_i64, name: "café" })
        .exec(&mut db)
        .await
        .unwrap();

    let items: Vec<Item> = Item::filter(Item::fields().name().ilike("CAFÉ".to_string()))
        .exec(&mut db)
        .await
        .unwrap();

    assert_eq!(
        items.len(),
        1,
        "ilike(\"CAFÉ\") should match \"café\" — got {} rows",
        items.len()
    );
}
