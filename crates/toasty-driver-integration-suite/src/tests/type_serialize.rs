use crate::prelude::*;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use toasty_core::{
    driver::{Operation, Rows},
    stmt::{ExprSet, Statement, Value},
};

#[driver_test(id(ID))]
pub async fn serialize_vec_string(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[serialize(json)]
        tags: Vec<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    // Insert — driver receives JSON string
    t.log().clear();
    let tags = vec!["rust".to_string(), "toasty".to_string()];
    let expected_json = serde_json::to_string(&tags).unwrap();
    let mut record = Item::create().tags(tags.clone()).exec(&mut db).await?;

    let (op, _) = t.log().pop();
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Insert(_ {
            source.body: ExprSet::Values(_ {
                rows: [=~ (Any, expected_json)],
                ..
            }),
            ..
        }),
        ..
    }));

    assert_eq!(Item::get_by_id(&mut db, &record.id).await?.tags, tags);

    // Update — driver receives JSON string
    t.log().clear();
    let new_tags = vec!["b".to_string(), "c".to_string()];
    let expected_json = serde_json::to_string(&new_tags).unwrap();
    record.update().tags(new_tags.clone()).exec(&mut db).await?;

    let (op, resp) = t.log().pop();
    if t.capability().sql {
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Update(_ {
                assignments: #{ [1_usize]: _ { expr: == expected_json, .. }},
                ..
            }),
            ..
        }));
    } else {
        assert_struct!(op, Operation::UpdateByKey(_ {
            assignments: #{ [1_usize]: _ { expr: == expected_json, .. }},
            ..
        }));
    }
    assert_struct!(resp, _ { rows: Rows::Count(1), .. });

    assert_eq!(Item::get_by_id(&mut db, &record.id).await?.tags, new_tags);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn serialize_nullable(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[serialize(json, nullable)]
        data: Option<HashMap<String, String>>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    // Some — driver receives JSON string
    t.log().clear();
    let map = HashMap::from([("key".to_string(), "value".to_string())]);
    let expected_json = serde_json::to_string(&map).unwrap();
    let record = Item::create().data(Some(map.clone())).exec(&mut db).await?;

    let (op, _) = t.log().pop();
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Insert(_ {
            source.body: ExprSet::Values(_ {
                rows: [=~ (Any, expected_json)],
                ..
            }),
            ..
        }),
        ..
    }));

    assert_eq!(Item::get_by_id(&mut db, &record.id).await?.data, Some(map));

    // None — driver receives SQL NULL
    t.log().clear();
    let empty_record = Item::create().data(None).exec(&mut db).await?;

    let (op, _) = t.log().pop();
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Insert(_ {
            source.body: ExprSet::Values(_ {
                rows: [=~ (Any, Value::Null)],
                ..
            }),
            ..
        }),
        ..
    }));

    assert_eq!(Item::get_by_id(&mut db, &empty_record.id).await?.data, None);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn serialize_non_nullable_option(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[serialize(json)]
        extra: Option<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    // None → JSON text "null", not SQL NULL
    t.log().clear();
    let empty_record = Item::create().extra(None).exec(&mut db).await?;

    let (op, _) = t.log().pop();
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Insert(_ {
            source.body: ExprSet::Values(_ {
                rows: [=~ (Any, "null")],
                ..
            }),
            ..
        }),
        ..
    }));

    assert_eq!(
        Item::get_by_id(&mut db, &empty_record.id).await?.extra,
        None
    );

    // Some → JSON string with quotes
    t.log().clear();
    let expected_json = serde_json::to_string(&Some("hello")).unwrap();
    let record = Item::create()
        .extra(Some("hello".to_string()))
        .exec(&mut db)
        .await?;

    let (op, _) = t.log().pop();
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Insert(_ {
            source.body: ExprSet::Values(_ {
                rows: [=~ (Any, expected_json)],
                ..
            }),
            ..
        }),
        ..
    }));

    assert_eq!(
        Item::get_by_id(&mut db, &record.id).await?.extra,
        Some("hello".to_string())
    );

    Ok(())
}

#[driver_test(id(ID))]
pub async fn serialize_custom_struct(t: &mut Test) -> Result<(), BoxError> {
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
        #[serialize(json)]
        meta: Metadata,
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
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Insert(_ {
            source.body: ExprSet::Values(_ {
                rows: [=~ (Any, expected_json)],
                ..
            }),
            ..
        }),
        ..
    }));

    assert_eq!(Item::get_by_id(&mut db, &record.id).await?.meta, meta);

    Ok(())
}
