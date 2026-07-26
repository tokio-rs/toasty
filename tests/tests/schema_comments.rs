#![cfg(feature = "sqlite")]

#[derive(Debug, toasty::Embed)]
struct Address {
    street: String,

    #[comment = "Postal city"]
    city: String,
}

#[derive(Debug, toasty::Model)]
#[comment = "User accounts"]
struct CommentedUser {
    #[key]
    #[auto]
    #[comment = "Stable identifier"]
    id: i64,

    address: Address,
}

#[tokio::test]
async fn model_comments_reach_the_database_schema() {
    let db = toasty::Db::builder()
        .models(toasty::models!(CommentedUser))
        .build(toasty_driver_sqlite::Sqlite::in_memory())
        .await
        .unwrap();

    let table = &db.schema().db.tables[0];
    assert_eq!(table.comment.as_deref(), Some("User accounts"));
    assert_eq!(
        table.columns[0].comment.as_deref(),
        Some("Stable identifier")
    );

    let city = table
        .columns
        .iter()
        .find(|column| column.name == "address_city")
        .unwrap();
    assert_eq!(city.comment.as_deref(), Some("Postal city"));
}
