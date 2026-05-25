use crate::prelude::*;

#[driver_test(requires(sql))]
pub async fn statement_and_query_on_db(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,
        #[index]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;
    let table = table_name(&db, "users");

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    let count = toasty::sql::statement(format!(
        "UPDATE {table} SET name = {} WHERE name = {}",
        placeholder(&db, 1),
        placeholder(&db, 2),
    ))
    .bind("Bob")
    .bind("Alice")
    .exec(&mut db)
    .await?;
    assert_eq!(count, 1);

    let rows = toasty::sql::query(format!("SELECT name FROM {table}"))
        .exec(&mut db)
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(field(&rows[0], 0), &toasty::stmt::Value::from("Bob"));

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn query_on_connection(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,
        #[index]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;
    let table = table_name(&db, "users");

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    let mut conn = db.connection().await?;
    let rows = toasty::sql::query(format!(
        "SELECT name FROM {table} WHERE name = {}",
        placeholder(&db, 1),
    ))
    .bind("Alice")
    .exec(&mut conn)
    .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(field(&rows[0], 0), &toasty::stmt::Value::from("Alice"));

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn statement_inside_transaction_rolls_back(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,
        #[index]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;
    let table = table_name(&db, "users");

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    let mut tx = db.transaction().await?;
    toasty::sql::statement(format!("UPDATE {table} SET name = 'Bob'"))
        .exec(&mut tx)
        .await?;
    tx.rollback().await?;

    let user = User::filter_by_name("Alice").get(&mut db).await?;
    assert_eq!(user.name, "Alice");

    Ok(())
}

#[driver_test(requires(not(sql)))]
pub async fn raw_sql_rejects_non_sql_drivers(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: uuid::Uuid,
        name: String,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let err = toasty::sql::statement("SELECT 1")
        .exec(&mut db)
        .await
        .unwrap_err();

    assert!(
        err.is_unsupported_feature(),
        "expected UnsupportedFeature, got: {err}"
    );

    Ok(())
}

fn table_name(db: &toasty::Db, suffix: &str) -> String {
    db.schema()
        .db
        .tables
        .iter()
        .find(|table| table.name.ends_with(suffix))
        .map(|table| table.name.clone())
        .unwrap_or_else(|| panic!("table {suffix} not found"))
}

fn placeholder(db: &toasty::Db, index: usize) -> String {
    render_placeholder(
        db.capability()
            .sql_placeholder
            .expect("SQL driver has placeholders"),
        index,
    )
}

fn render_placeholder(placeholder: toasty::SqlPlaceholder, index: usize) -> String {
    match placeholder {
        toasty::SqlPlaceholder::QuestionMark => "?".to_string(),
        toasty::SqlPlaceholder::NumberedQuestionMark => format!("?{index}"),
        toasty::SqlPlaceholder::DollarNumber => format!("${index}"),
    }
}

fn field(value: &toasty::stmt::Value, index: usize) -> &toasty::stmt::Value {
    let toasty::stmt::Value::Record(record) = value else {
        panic!("expected record, got {value:?}");
    };

    &record[index]
}
