//! Test keyset pagination over a multi-column `order_by`.
//!
//! The cursor must be compared lexicographically: a row that ties the cursor
//! on a leading sort column but is beyond it on a later column belongs on the
//! next page. These are gated on `sql` because SQL drivers rewrite the cursor
//! into a WHERE filter; NoSQL drivers use a driver-level cursor instead.

use crate::prelude::*;
use toasty::{
    Executor,
    stmt::{IntoStatement, Page},
};
use toasty_core::stmt::{self, Value};

fn cursor_len(cursor: Option<&Value>) -> usize {
    let Some(Value::Record(cursor)) = cursor else {
        panic!("expected a record cursor");
    };
    cursor.fields.len()
}

#[driver_test(requires(and(sql, backward_pagination)))]
pub async fn paginate_first_page_has_no_previous_page(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;
    toasty::create!(Item::[{ id: 1 }, { id: 2 }])
        .exec(&mut db)
        .await?;

    let first: Page<Item> = Item::all()
        .order_by(Item::fields().id().asc())
        .paginate(1)
        .exec(&mut db)
        .await?;

    assert!(!first.has_prev());
    assert!(first.prev(&mut db).await?.is_none());

    Ok(())
}

#[driver_test(requires(and(sql, backward_pagination)))]
pub async fn paginate_first_page_ignores_subquery_cursor(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;
    toasty::create!(Item::[{ id: 1 }, { id: 2 }])
        .exec(&mut db)
        .await?;

    let mut inner = Item::all()
        .order_by(Item::fields().id().asc())
        .select((Item::fields().id(), Item::fields().id()))
        .into_statement()
        .into_untyped()
        .into_query_unwrap();
    inner.limit = Some(stmt::Limit::Cursor(stmt::LimitCursor {
        page_size: stmt::Value::from(2_i64).into(),
        after: Some(stmt::Value::from(0_i64).into()),
    }));

    let mut statement = Item::all()
        .order_by(Item::fields().id().asc())
        .into_statement()
        .into_untyped();

    let outer = statement.as_query_mut().unwrap();
    outer.limit = Some(stmt::Limit::Cursor(stmt::LimitCursor {
        page_size: stmt::Value::from(1_i64).into(),
        after: None,
    }));
    outer.with = Some(stmt::With {
        ctes: vec![stmt::Cte { query: inner }],
    });

    let response = db.exec_untyped(statement).await?;
    assert!(response.prev_cursor.is_none());

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn paginate_single_non_unique_column(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: i64,
        group: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;
    toasty::create!(Item::[
        { id: 1, group: 1 },
        { id: 2, group: 1 },
        { id: 3, group: 1 },
        { id: 4, group: 1 },
    ])
    .exec(&mut db)
    .await?;

    let first: Page<Item> = Item::all()
        .order_by(Item::fields().group().asc())
        .paginate(2)
        .exec(&mut db)
        .await?;
    assert_struct!(first.items, [_ { id: 1, .. }, _ { id: 2, .. }]);
    assert_eq!(cursor_len(first.next_cursor.as_ref()), 2);

    let second = first.next(&mut db).await?.unwrap();
    assert_struct!(second.items, [_ { id: 3, .. }, _ { id: 4, .. }]);

    let first_again = second.prev(&mut db).await?.unwrap();
    assert_struct!(first_again.items, [_ { id: 1, .. }, _ { id: 2, .. }]);

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn paginate_non_unique_column_with_newtype_primary_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct ItemId(i64);

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: ItemId,
        group: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;
    toasty::create!(Item::[
        { id: ItemId(1), group: 1 },
        { id: ItemId(2), group: 1 },
        { id: ItemId(3), group: 1 },
    ])
    .exec(&mut db)
    .await?;

    let first: Page<Item> = Item::all()
        .order_by(Item::fields().group().asc())
        .paginate(2)
        .exec(&mut db)
        .await?;
    assert_eq!(
        first.items.iter().map(|item| item.id.0).collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(cursor_len(first.next_cursor.as_ref()), 2);

    let second = first.next(&mut db).await?.unwrap();
    assert_eq!(
        second
            .items
            .iter()
            .map(|item| item.id.0)
            .collect::<Vec<_>>(),
        [3]
    );

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn paginate_accepts_cursor_without_hidden_primary_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: i64,
        group: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;
    toasty::create!(Item::[
        { id: 1, group: 1 },
        { id: 2, group: 1 },
        { id: 3, group: 1 },
    ])
    .exec(&mut db)
    .await?;

    let first: Page<Item> = Item::all()
        .order_by(Item::fields().group().asc())
        .paginate(2)
        .after(0_i64)
        .exec(&mut db)
        .await?;
    assert_struct!(first.items, [_ { id: 1, .. }, _ { id: 2, .. }]);
    assert_eq!(cursor_len(first.next_cursor.as_ref()), 2);

    let second = first.next(&mut db).await?.unwrap();
    assert_struct!(second.items, [_ { id: 3, .. }]);

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn paginate_unique_non_nullable_column_needs_no_tiebreaker(
    test: &mut Test,
) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: i64,
        #[unique]
        rank: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;
    toasty::create!(Item::[
        { id: 1, rank: 10 },
        { id: 2, rank: 20 },
        { id: 3, rank: 30 },
    ])
    .exec(&mut db)
    .await?;

    let page: Page<Item> = Item::all()
        .order_by(Item::fields().rank().asc())
        .paginate(2)
        .exec(&mut db)
        .await?;
    assert_eq!(cursor_len(page.next_cursor.as_ref()), 1);

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn paginate_composite_unique_non_nullable_order_needs_no_tiebreaker(
    test: &mut Test,
) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[unique(group, rank)]
    struct Item {
        #[key]
        id: i64,
        group: i64,
        rank: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;
    toasty::create!(Item::[
        { id: 1, group: 1, rank: 10 },
        { id: 2, group: 1, rank: 20 },
        { id: 3, group: 2, rank: 10 },
    ])
    .exec(&mut db)
    .await?;

    let page: Page<Item> = Item::all()
        .order_by((Item::fields().group().asc(), Item::fields().rank().asc()))
        .paginate(2)
        .exec(&mut db)
        .await?;
    assert_eq!(cursor_len(page.next_cursor.as_ref()), 2);

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn paginate_nullable_unique_column_uses_primary_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: i64,
        #[unique]
        rank: Option<i64>,
    }

    let mut db = test.setup_db(models!(Item)).await;
    toasty::create!(Item::[
        { id: 1, rank: None },
        { id: 2, rank: None },
        { id: 3, rank: Some(10) },
        { id: 4, rank: Some(20) },
    ])
    .exec(&mut db)
    .await?;

    let mut page: Page<Item> = Item::all()
        .order_by(Item::fields().rank().asc())
        .paginate(1)
        .exec(&mut db)
        .await?;
    assert_eq!(cursor_len(page.next_cursor.as_ref()), 2);

    let mut ids = Vec::new();
    loop {
        ids.extend(page.iter().map(|item| item.id));
        let Some(next) = page.next(&mut db).await? else {
            break;
        };
        page = next;
    }
    ids.sort_unstable();
    assert_eq!(ids, [1, 2, 3, 4]);

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn paginate_multi_column_equal_leading_values(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: i64,
        group: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;

    // All rows share the same leading sort value.
    toasty::create!(Item::[
        { id: 1, group: 1 },
        { id: 2, group: 1 },
        { id: 3, group: 1 },
    ])
    .exec(&mut db)
    .await?;

    let page: Page<Item> = Item::all()
        .order_by((Item::fields().group().asc(), Item::fields().id().asc()))
        .paginate(2)
        .exec(&mut db)
        .await?;
    assert_struct!(page.items, [_ { id: 1, .. }, _ { id: 2, .. }]);

    let page: Page<Item> = page.next(&mut db).await?.unwrap();
    assert_struct!(page.items, [_ { id: 3, .. }]);

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn paginate_multi_column_mixed_directions(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: i64,
        group: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { id: 1, group: 1 },
        { id: 2, group: 1 },
        { id: 3, group: 2 },
        { id: 4, group: 2 },
    ])
    .exec(&mut db)
    .await?;

    // group desc, id asc: 3, 4, 1, 2
    let page: Page<Item> = Item::all()
        .order_by((Item::fields().group().desc(), Item::fields().id().asc()))
        .paginate(2)
        .exec(&mut db)
        .await?;
    assert_struct!(page.items, [_ { id: 3, .. }, _ { id: 4, .. }]);

    let page: Page<Item> = page.next(&mut db).await?.unwrap();
    assert_struct!(page.items, [_ { id: 1, .. }, _ { id: 2, .. }]);

    Ok(())
}
