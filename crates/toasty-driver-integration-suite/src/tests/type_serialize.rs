use crate::prelude::*;

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use toasty::Json;
use toasty_core::{
    driver::{Operation, Rows},
    stmt::{Assignment, Expr, ExprSet, Statement, Value},
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

#[driver_test(id(ID))]
pub async fn json_vec_string(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
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
