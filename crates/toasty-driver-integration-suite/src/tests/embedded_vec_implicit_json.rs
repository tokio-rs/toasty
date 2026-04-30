use crate::prelude::*;

use toasty_core::{
    driver::{Operation, Rows},
    stmt::{Assignment, Expr, ExprSet, Statement, Value},
};

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
pub async fn vec_string_implicit_json(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        tags: Vec<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    t.log().clear();
    let tags = vec!["rust".to_string(), "toasty".to_string()];
    let expected_json = serde_json::to_string(&tags).unwrap();
    let mut record = Item::create().tags(tags.clone()).exec(&mut db).await?;

    let (op, _) = t.log().pop();
    let val_pos = if driver_test_cfg!(id_u64) { 0 } else { 1 };
    assert_insert_serialized(t, &op, val_pos, &expected_json);

    assert_eq!(Item::get_by_id(&mut db, &record.id).await?.tags, tags);

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

    assert_eq!(Item::get_by_id(&mut db, &record.id).await?.tags, new_tags);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn vec_i64_implicit_json(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        scores: Vec<i64>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let scores = vec![1i64, 2, 3, -4];
    let record = Item::create().scores(scores.clone()).exec(&mut db).await?;

    assert_eq!(Item::get_by_id(&mut db, &record.id).await?.scores, scores);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn option_vec_string_implicit_json(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        tags: Option<Vec<String>>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let none_record = Item::create().tags(None).exec(&mut db).await?;
    let (op, _) = t.log().pop();
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Insert({
            source.body: ExprSet::Values({
                rows: [=~ (Any, Value::Null)],
            }),
        }),
    }));
    assert_eq!(Item::get_by_id(&mut db, &none_record.id).await?.tags, None);

    let tags = vec!["alpha".to_string()];
    let some_record = Item::create()
        .tags(Some(tags.clone()))
        .exec(&mut db)
        .await?;
    assert_eq!(
        Item::get_by_id(&mut db, &some_record.id).await?.tags,
        Some(tags),
    );

    Ok(())
}
