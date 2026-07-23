use crate::prelude::*;

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use toasty::Json;
use toasty_core::{
    driver::{Operation, Rows},
    schema::db,
    stmt::{Assignment, Expr, ExprSet, InsertTarget, Source, Statement, Type, UpdateTarget, Value},
};

/// Assert the INSERT emitted a single row whose last column carries `expected`
/// (as a JSON-serialized string). Covers both SQL (bind parameter at `pos`)
/// and non-SQL (inline value) representations.
fn assert_insert_serialized(t: &Test, op: &Operation, pos: usize, expected: &str) {
    let sql = t.capability().sql;
    let val_pat = if sql {
        ArgOr::Arg(pos)
    } else {
        ArgOr::Value(expected)
    };
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Insert({
            source.body: ExprSet::Values({
                rows: [=~ (Any, val_pat)],
            }),
        }),
    }));
    if sql {
        assert_struct!(op, Operation::QuerySql({
            params[pos].value: == expected,
        }));
    }
}

fn assert_native_json_insert(
    op: &Operation,
    table: db::TableId,
    storage_ty: db::Type,
    expected: &str,
) {
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Insert({
            target: InsertTarget::Table({
                table: == table,
            }),
            source.body: ExprSet::Values({
                rows: [=~ (Any, ArgOr::<Any>::Arg(0))],
            }),
        }),
        params: [{
            value: == expected,
            ty: == storage_ty,
        }],
    }));
}

fn assert_native_json_query(op: &Operation, table: db::TableId) {
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Query({
            body: ExprSet::Select({
                source: Source::Table({
                    tables: [== table, ..],
                }),
            }),
        }),
        ret: Some([Type::U64, Type::String]),
    }));
}

fn assert_native_json_update(
    op: &Operation,
    table: db::TableId,
    storage_ty: db::Type,
    expected: &str,
) {
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Update({
            target: UpdateTarget::Table(== table),
            assignments: #{ [1]: Assignment::Set(Expr::Arg({ position: 0 }))},
        }),
        params: [{
            value: == expected,
            ty: == storage_ty,
        }, ..],
        ret: None,
    }));
}

#[driver_test(id(ID))]
pub async fn json_vec_string(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[column(type = text)]
        tags: Json<Vec<String>>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    // Insert — driver receives JSON string. The bare `Vec<String>` is
    // accepted via the `IntoExpr<Json<T>> for T` blanket on `Json<T>`,
    // so callers don't need to spell `Json(value)` at setter sites.
    t.log().clear();
    let tags = vec!["rust".to_string(), "toasty".to_string()];
    let expected_json = serde_json::to_string(&tags).unwrap();
    let mut record = Item::create().tags(tags.clone()).exec(&mut db).await?;

    let (op, _) = t.log().pop();
    let val_pos = if driver_test_cfg!(id_u64) { 0 } else { 1 };
    assert_insert_serialized(t, &op, val_pos, &expected_json);

    assert_eq!(
        Item::get_by_id(&mut db, &record.id).await?.tags,
        Json(tags.clone())
    );

    // Update — same blanket lets the update builder take the bare value.
    t.log().clear();
    let new_tags = vec!["b".to_string(), "c".to_string()];
    let expected_json = serde_json::to_string(&new_tags).unwrap();
    record.update().tags(new_tags.clone()).exec(&mut db).await?;

    let (op, resp) = t.log().pop();
    if t.capability().sql {
        assert_struct!(op, Operation::QuerySql({
            stmt: Statement::Update({
                assignments: #{ [1]: Assignment::Set(Expr::Arg({ position: 0 }))},
            }),
            params[0].value: == expected_json,
        }));
    } else {
        assert_struct!(op, Operation::UpdateByKey({
            assignments: #{ [1]: Assignment::Set(== expected_json.as_str())},
        }));
    }
    assert_struct!(resp, { values: Rows::Count(1) });

    assert_eq!(
        Item::get_by_id(&mut db, &record.id).await?.tags,
        Json(new_tags)
    );

    Ok(())
}

#[driver_test(id(ID))]
pub async fn json_option_outside_sql_null(t: &mut Test) -> Result<(), BoxError> {
    // `Option<Json<T>>` — None maps to SQL `NULL`, Some(v) to a JSON string.
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[column(type = text)]
        data: Option<Json<HashMap<String, String>>>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    // Some — driver receives JSON string. The `Option<Json<T>>` field
    // also accepts a bare `Json(value)` via the `IntoExpr<Option<T>> for T`
    // blanket, so the `Some(...)` wrapping is optional.
    t.log().clear();
    let map = HashMap::from([("key".to_string(), "value".to_string())]);
    let expected_json = serde_json::to_string(&map).unwrap();
    let record = Item::create().data(Json(map.clone())).exec(&mut db).await?;

    let (op, _) = t.log().pop();
    let val_pos = if driver_test_cfg!(id_u64) { 0 } else { 1 };
    assert_insert_serialized(t, &op, val_pos, &expected_json);

    assert_eq!(
        Item::get_by_id(&mut db, &record.id).await?.data,
        Some(Json(map))
    );

    // None — driver receives SQL NULL.
    t.log().clear();
    let empty_record = Item::create().data(None).exec(&mut db).await?;

    let (op, _) = t.log().pop();
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Insert({
            source.body: ExprSet::Values({
                rows: [=~ (Any, Value::Null)],
            }),
        }),
    }));

    assert_eq!(Item::get_by_id(&mut db, &empty_record.id).await?.data, None);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn json_option_inside_json_null(t: &mut Test) -> Result<(), BoxError> {
    // `Json<Option<T>>` — None maps to the JSON literal `"null"`, not SQL `NULL`.
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[column(type = text)]
        extra: Json<Option<String>>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    // None → JSON text "null", not SQL NULL.
    // `Option<String>` decodes through the `IntoExpr<Json<T>> for T`
    // blanket — `T` here is `Option<String>`, which is `Serialize`.
    t.log().clear();
    let empty_record = Item::create().extra(None).exec(&mut db).await?;

    let (op, _) = t.log().pop();
    let val_pos = if driver_test_cfg!(id_u64) { 0 } else { 1 };
    assert_insert_serialized(t, &op, val_pos, "null");

    assert_eq!(
        Item::get_by_id(&mut db, &empty_record.id).await?.extra,
        Json(None)
    );

    // Some → JSON string with quotes
    t.log().clear();
    let expected_json = serde_json::to_string(&Some("hello")).unwrap();
    let record = Item::create()
        .extra(Some("hello".to_string()))
        .exec(&mut db)
        .await?;

    let (op, _) = t.log().pop();
    let val_pos = if driver_test_cfg!(id_u64) { 0 } else { 1 };
    assert_insert_serialized(t, &op, val_pos, &expected_json);

    assert_eq!(
        Item::get_by_id(&mut db, &record.id).await?.extra,
        Json(Some("hello".to_string()))
    );

    Ok(())
}

#[driver_test(id(ID))]
pub async fn json_custom_struct(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Metadata {
        version: u32,
        labels: Vec<String>,
    }

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[column(type = text)]
        meta: Json<Metadata>,
    }

    let mut db = t.setup_db(models!(Item)).await;
    t.log().clear();

    let meta = Metadata {
        version: 42,
        labels: vec!["alpha".to_string(), "beta".to_string()],
    };
    let expected_json = serde_json::to_string(&meta).unwrap();
    let record = Item::create().meta(meta.clone()).exec(&mut db).await?;

    let (op, _) = t.log().pop();
    let val_pos = if driver_test_cfg!(id_u64) { 0 } else { 1 };
    assert_insert_serialized(t, &op, val_pos, &expected_json);

    assert_eq!(Item::get_by_id(&mut db, &record.id).await?.meta, Json(meta));

    Ok(())
}

#[driver_test(requires(native_json))]
pub async fn json_native_round_trip(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Metadata {
        title: String,
        labels: Vec<String>,
    }

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        #[column(type = json)]
        metadata: Json<Metadata>,
    }

    let mut db = t.setup_db(models!(Item)).await;
    assert_eq!(column_storage_ty(&db, "items", "metadata"), db::Type::Json);
    let item_table = table_id(&db, "items");

    let metadata = Metadata {
        title: "quoted \"text\" and 日本語".to_string(),
        labels: vec!["one".to_string(), "two".to_string()],
    };
    let expected_json = serde_json::to_string(&metadata).unwrap();
    t.log().clear();
    let mut item = toasty::create!(Item {
        metadata: Json(metadata.clone()),
    })
    .exec(&mut db)
    .await?;

    let (op, _) = t.log().pop();
    assert_native_json_insert(&op, item_table, db::Type::Json, &expected_json);
    assert!(t.log().is_empty());

    let read = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(read.metadata, Json(metadata));
    let (op, _) = t.log().pop();
    assert_native_json_query(&op, item_table);
    assert!(t.log().is_empty());

    let updated = Metadata {
        title: "updated".to_string(),
        labels: vec![],
    };
    let expected_json = serde_json::to_string(&updated).unwrap();
    item.update()
        .metadata(updated.clone())
        .exec(&mut db)
        .await?;

    let (op, resp) = t.log().pop();
    assert_native_json_update(&op, item_table, db::Type::Json, &expected_json);
    assert_struct!(resp, { values: Rows::Count(1) });
    assert!(t.log().is_empty());

    let read = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(read.metadata, Json(updated));
    let (op, _) = t.log().pop();
    assert_native_json_query(&op, item_table);
    assert!(t.log().is_empty());

    Ok(())
}

#[driver_test(requires(native_json))]
pub async fn json_native_nulls(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        #[column(type = json)]
        sql_null: Option<Json<String>>,
        #[column(type = json)]
        json_null: Json<Option<String>>,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let item = toasty::create!(Item {
        sql_null: None,
        json_null: Json(None),
    })
    .exec(&mut db)
    .await?;
    let item = Item::get_by_id(&mut db, &item.id).await?;

    assert_eq!(item.sql_null, None);
    assert_eq!(item.json_null, Json(None));

    Ok(())
}

#[driver_test(requires(native_jsonb))]
pub async fn jsonb_native_round_trip(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        #[column(type = "jsonb")]
        payload: Json<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;
    assert_eq!(column_storage_ty(&db, "items", "payload"), db::Type::Jsonb);
    let item_table = table_id(&db, "items");

    let payload = "quoted \"text\" and 日本語".to_string();
    let expected_json = serde_json::to_string(&payload).unwrap();
    t.log().clear();
    let item = toasty::create!(Item {
        payload: Json(payload.clone()),
    })
    .exec(&mut db)
    .await?;

    let (op, _) = t.log().pop();
    assert_native_json_insert(&op, item_table, db::Type::Jsonb, &expected_json);
    assert!(t.log().is_empty());

    let read = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(read.payload, Json(payload));
    let (op, _) = t.log().pop();
    assert_native_json_query(&op, item_table);
    assert!(t.log().is_empty());

    Ok(())
}

#[driver_test(id(ID))]
pub async fn json_data_enum_field(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Payload {
        Data {
            #[column(type = text)]
            tags: Json<Vec<String>>,
        },
        Empty,
    }

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        payload: Payload,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let tags = vec!["rust".to_string(), "toasty".to_string()];

    let item = toasty::create!(Item {
        payload: Payload::Data {
            tags: Json(tags.clone()),
        },
    })
    .exec(&mut db)
    .await?;

    assert_eq!(
        Item::get_by_id(&mut db, &item.id).await?.payload,
        Payload::Data { tags: Json(tags) }
    );

    Ok(())
}
