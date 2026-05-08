//! Tests for `IN`/`NOT IN` list filters.
//!
//! On PostgreSQL the engine rewrites `expr IN (...)` to `expr = ANY($1)` and
//! `expr NOT IN (...)` to `expr <> ALL($1)`, binding the list as a single
//! array parameter. Other SQL drivers continue to expand the list into one
//! parameter per item. These tests cover both code paths via a single
//! `Item` scenario, with each test exercising a different element type or
//! list shape.
//!
//! Assertions on the issued driver op are gated on `bind_list_param +
//! predicate_match_any`: when both are on (PostgreSQL) a single
//! `Value::List` param typed `List(elem)` is expected; otherwise N scalar
//! params each typed `elem`.

use crate::prelude::*;

use toasty_core::{
    driver::{Capability, Operation, operation::QuerySql},
    schema::db,
    stmt,
};

#[driver_test(id(ID), scenario(crate::scenarios::in_list_item))]
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

    t.log().clear();

    let items = Item::filter(Item::fields().name().in_list(["a", "c"]))
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 2);
    let mut names: Vec<_> = items.iter().map(|i| i.name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["a".to_string(), "c".to_string()]);

    if t.capability().sql {
        let elem = column_storage_ty(&db, "items", "name");
        assert_in_list_bind(&pop_select(t), t.capability(), &elem, Some(2));
    }

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::in_list_item))]
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

    t.log().clear();

    // `NOT IN [a, c]` → on PG this lowers to `name <> ALL($1)`.
    let items = Item::filter(Item::fields().name().in_list(["a", "c"]).not())
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 2);
    let mut names: Vec<_> = items.iter().map(|i| i.name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["b".to_string(), "d".to_string()]);

    if t.capability().sql {
        let elem = column_storage_ty(&db, "items", "name");
        assert_in_list_bind(&pop_select(t), t.capability(), &elem, Some(2));
    }

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::in_list_item))]
pub async fn in_list_empty(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(Item::[
        { name: "a", n: 1 },
        { name: "b", n: 2 },
    ])
    .exec(&mut db)
    .await?;

    t.log().clear();

    // Empty list — `IN ()` is unsatisfiable; the simplifier folds the
    // predicate to `false` and the engine short-circuits before issuing
    // any driver op. If this regresses and an empty list reached
    // extract_params, `finalize_ty` would panic on `List(Unknown)` (no
    // column refinement runs on a folded branch).
    let empty: Vec<String> = vec![];
    let items = Item::filter(Item::fields().name().in_list(empty))
        .exec(&mut db)
        .await?;

    assert!(items.is_empty(), "IN () must match nothing, got {items:?}");
    assert!(
        t.log().is_empty(),
        "empty IN () should short-circuit before issuing any driver op",
    );

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::in_list_item))]
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

    if t.capability().sql {
        let elem = column_storage_ty(&db, "items", "n");
        assert_in_list_bind(&pop_select(t), t.capability(), &elem, Some(expected));
    }

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::in_list_item))]
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

    t.log().clear();

    let found = Item::filter(Item::fields().id().in_list(needles))
        .exec(&mut db)
        .await?;

    assert_eq!(found.len(), 2);
    let mut names: Vec<_> = found.iter().map(|i| i.name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["a".to_string(), "c".to_string()]);

    if t.capability().sql {
        let elem = column_storage_ty(&db, "items", "id");
        assert_in_list_bind(&pop_select(t), t.capability(), &elem, Some(2));
    }

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::in_list_item))]
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

    t.log().clear();

    let needles: Vec<Option<String>> = vec![Some("rusty".to_string()), None];
    let items = Item::filter(Item::fields().bio().in_list(needles))
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "a");

    if t.capability().sql {
        // The per-item path's scalar count is implementation-defined here
        // (the engine may drop the null operand at extract time), so pass
        // `None` to skip the count check; element-type assertions still run.
        let elem = column_storage_ty(&db, "items", "bio");
        assert_in_list_bind(&pop_select(t), t.capability(), &elem, None);
    }

    Ok(())
}

// ============================================================================
// Helpers
// ============================================================================

/// Pop ops from the log until the SELECT `QuerySql` is found.
#[track_caller]
fn pop_select(t: &mut Test) -> QuerySql {
    while !t.log().is_empty() {
        if let Operation::QuerySql(q) = t.log().pop_op()
            && matches!(q.stmt, stmt::Statement::Query(_))
        {
            return q;
        }
    }
    panic!("expected a SELECT QuerySql op in the log");
}

/// Look up the storage type of a column from the schema. Drives element-
/// type assertions per driver, since the same model maps to different
/// storage types (e.g. `String` → `Text` on PG/SQLite vs. `VarChar(191)`
/// on MySQL).
fn column_storage_ty(db: &toasty::Db, table_name: &str, column_name: &str) -> db::Type {
    let schema = db.schema();
    let table = schema
        .db
        .tables
        .iter()
        .find(|t| t.name == table_name || t.name.ends_with(table_name))
        .unwrap_or_else(|| panic!("table '{table_name}' not in schema"));
    table
        .columns
        .iter()
        .find(|c| c.name == column_name)
        .unwrap_or_else(|| panic!("column '{column_name}' not in table '{table_name}'"))
        .storage_ty
        .clone()
}

/// Assert that an `IN`-list query was bound according to the driver's
/// capabilities. With both `bind_list_param` and `predicate_match_any` on
/// (PostgreSQL), expects a single `Value::List` param typed
/// `List(elem)`. Otherwise expects `expected_scalars` scalar params, each
/// typed `elem`.
#[track_caller]
fn assert_in_list_bind(
    query: &QuerySql,
    cap: &Capability,
    elem: &db::Type,
    expected_scalars: Option<usize>,
) {
    if cap.bind_list_param && cap.predicate_match_any {
        let expected_ty = db::Type::List(Box::new(elem.clone()));
        assert_struct!(query, {
            params: [{
                value: stmt::Value::List(_),
                ty: == expected_ty,
            }],
        });
    } else {
        if let Some(n) = expected_scalars {
            assert_eq!(query.params.len(), n);
        }
        for (i, p) in query.params.iter().enumerate() {
            assert_eq!(
                &p.ty, elem,
                "params[{i}].ty should be the column storage type"
            );
        }
    }
}
