//! Tests for `IN`/`NOT IN` list filters.
//!
//! On PostgreSQL the engine rewrites `expr IN (...)` to `expr = ANY($1)` and
//! `expr NOT IN (...)` to `expr <> ALL($1)`, binding the list as a single
//! array parameter. Other SQL drivers continue to expand the list into one
//! parameter per item. These tests cover both code paths via a single
//! `Item` scenario, with each test exercising a different element type or
//! list shape.

use crate::prelude::*;

use toasty_core::{driver::Operation, stmt};

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::in_list_item))]
pub async fn in_list_string(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(Item::[
        { name: "a", n: 1 },
        { name: "b", n: 2 },
        { name: "c", n: 3 },
        { name: "d", n: 4 },
    ])
    .exec(&mut db)
    .await?;

    let items = Item::filter(Item::fields().name().in_list(["a", "c"]))
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 2);
    let mut names: Vec<_> = items.iter().map(|i| i.name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["a".to_string(), "c".to_string()]);

    Ok(())
}

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::in_list_item))]
pub async fn not_in_list_string(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(Item::[
        { name: "a", n: 1 },
        { name: "b", n: 2 },
        { name: "c", n: 3 },
        { name: "d", n: 4 },
    ])
    .exec(&mut db)
    .await?;

    // `NOT IN [a, c]` → on PG this lowers to `name <> ALL($1)`.
    let items = Item::filter(Item::fields().name().in_list(["a", "c"]).not())
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 2);
    let mut names: Vec<_> = items.iter().map(|i| i.name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["b".to_string(), "d".to_string()]);

    Ok(())
}

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::in_list_item))]
pub async fn in_list_empty(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(Item::[
        { name: "a", n: 1 },
        { name: "b", n: 2 },
    ])
    .exec(&mut db)
    .await?;

    // Empty list — `IN ()` is unsatisfiable; the simplifier folds this to
    // `false` regardless of whether the gate would otherwise rewrite to
    // `= ANY($1)`.
    let empty: Vec<String> = vec![];
    let items = Item::filter(Item::fields().name().in_list(empty))
        .exec(&mut db)
        .await?;

    assert!(items.is_empty(), "IN () must match nothing, got {items:?}");

    Ok(())
}

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::in_list_item))]
pub async fn in_list_i64_large(t: &mut Test) -> Result<()> {
    // Regression guard: with PG's gate on, the engine must bind the whole
    // list as a single `Value::List` param. With the gate off (SQLite,
    // MySQL), the engine must still emit one bind per item.

    let mut db = setup(t).await;

    for n in 0..200_i64 {
        Item::create()
            .name(format!("item-{n}"))
            .n(n)
            .exec(&mut db)
            .await?;
    }

    let needles: Vec<i64> = (0..200_i64).step_by(2).collect();
    let expected = needles.len();

    t.log().clear();

    let items = Item::filter(Item::fields().n().in_list(needles))
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), expected);

    // Locate the SELECT op (drivers may emit setup queries first).
    let mut select_op = None;
    while !t.log().is_empty() {
        let op = t.log().pop_op();
        if matches!(&op, Operation::QuerySql(q) if matches!(q.stmt, stmt::Statement::Query(_))) {
            select_op = Some(op);
            break;
        }
    }
    let Some(Operation::QuerySql(query)) = select_op else {
        panic!("expected a SELECT QuerySql op");
    };

    let cap = t.capability();
    if cap.bind_list_param && cap.predicate_match_any {
        assert_eq!(
            query.params.len(),
            1,
            "expected 1 array param when predicate_match_any is on"
        );
        assert!(
            matches!(query.params[0].value, stmt::Value::List(_)),
            "expected the param to be a Value::List, got {:#?}",
            query.params[0].value
        );
    } else {
        assert_eq!(
            query.params.len(),
            expected,
            "expected one bind per item when predicate_match_any is off"
        );
    }

    Ok(())
}

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::in_list_item))]
pub async fn in_list_id(t: &mut Test) -> Result<()> {
    // Filter by the auto-generated id. Runs once per ID variant, so this
    // exercises the PG driver's per-element-type dispatch for both `u64`
    // (INT8) and `uuid::Uuid` (UUID) bind paths.

    let mut db = setup(t).await;

    let items = toasty::create!(Item::[
        { name: "a", n: 1 },
        { name: "b", n: 2 },
        { name: "c", n: 3 },
        { name: "d", n: 4 },
    ])
    .exec(&mut db)
    .await?;

    let needles = vec![items[0].id, items[2].id];
    let found = Item::filter(Item::fields().id().in_list(needles))
        .exec(&mut db)
        .await?;

    assert_eq!(found.len(), 2);
    let mut names: Vec<_> = found.iter().map(|i| i.name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["a".to_string(), "c".to_string()]);

    Ok(())
}

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::in_list_item))]
pub async fn in_list_with_null(t: &mut Test) -> Result<()> {
    // Exercises the PG driver's `Vec<Option<T>>` bind path: a `None` in the
    // list maps to a SQL NULL inside the bound array.
    //
    // SQL semantics: `bio IN ('rusty', NULL)` is true when `bio = 'rusty'`
    // and unknown (treated as false in WHERE) when `bio` is NULL or differs
    // — the NULL in the list never matches anything.

    let mut db = setup(t).await;

    toasty::create!(Item::[
        { name: "a", n: 1, bio: "rusty" },
        { name: "b", n: 2 },
        { name: "c", n: 3, bio: "databases" },
    ])
    .exec(&mut db)
    .await?;

    let needles: Vec<Option<String>> = vec![Some("rusty".to_string()), None];
    let items = Item::filter(Item::fields().bio().in_list(needles))
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "a");

    Ok(())
}
