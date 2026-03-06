//! Pagination driver-op assertions for DynamoDB (and SQL) paths.
//!
//! These tests verify that pagination parameters (limit, sort direction,
//! cursor) flow through the engine and are correctly set on the `QueryPk`
//! operation dispatched to NoSQL drivers.

use crate::prelude::*;

use toasty_core::{
    driver::{Operation, Rows},
    stmt::{self, ExprSet, Statement},
};

/// Query with a limit on a partitioned composite key dispatches `QueryPk` with
/// the `limit` field set on NoSQL drivers.
#[driver_test]
pub async fn limit_on_partition_query(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = user_id, local = order)]
    struct Todo {
        user_id: String,
        order: i64,
        title: String,
    }

    let mut db = test.setup_db(models!(Todo)).await;

    for i in 0..10 {
        Todo::create()
            .user_id("alice")
            .order(i)
            .title(format!("todo-{i}"))
            .exec(&mut db)
            .await?;
    }

    test.log().clear();

    let todo_table_id = table_id(&db, "todos");
    let is_sql = test.capability().sql;

    // Query with a limit — on NoSQL this must produce a QueryPk with limit set.
    let todos: Vec<_> = Todo::filter_by_user_id("alice")
        .limit(3)
        .collect(&mut db)
        .await?;

    assert_eq!(todos.len(), 3);

    let (op, resp) = test.log().pop();

    if is_sql {
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Query(_ {
                body: ExprSet::Select(_ { .. }),
                limit: Some(_),
                ..
            }),
            ..
        }));
    } else {
        assert_struct!(op, Operation::QueryPk(_ {
            table: == todo_table_id,
            limit: Some(3),
            ..
        }));
    }

    assert_struct!(resp.rows, Rows::Stream(_));

    Ok(())
}

/// Query with descending order_by on a partitioned composite key dispatches
/// `QueryPk` with `order: Some(Direction::Desc)` on NoSQL drivers.
#[driver_test]
pub async fn order_desc_on_partition_query(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = user_id, local = order)]
    struct Todo {
        user_id: String,
        order: i64,
        title: String,
    }

    let mut db = test.setup_db(models!(Todo)).await;

    for i in 0..5 {
        Todo::create()
            .user_id("alice")
            .order(i)
            .title(format!("todo-{i}"))
            .exec(&mut db)
            .await?;
    }

    test.log().clear();

    let todo_table_id = table_id(&db, "todos");
    let is_sql = test.capability().sql;

    // Query with descending order — on NoSQL, order must be Desc.
    let todos: Vec<_> = Todo::filter_by_user_id("alice")
        .order_by(Todo::fields().order().desc())
        .limit(3)
        .collect(&mut db)
        .await?;

    assert_eq!(todos.len(), 3);
    // Verify descending order
    assert!(todos[0].order > todos[1].order);
    assert!(todos[1].order > todos[2].order);

    let (op, _resp) = test.log().pop();

    if is_sql {
        assert_struct!(op, Operation::QuerySql(_ { .. }));
    } else {
        assert_struct!(op, Operation::QueryPk(_ {
            table: == todo_table_id,
            limit: Some(3),
            order: Some(stmt::Direction::Desc),
            ..
        }));
    }

    Ok(())
}

/// Query with ascending order_by on a partitioned composite key dispatches
/// `QueryPk` with `order: Some(Direction::Asc)` on NoSQL drivers.
#[driver_test]
pub async fn order_asc_on_partition_query(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = user_id, local = order)]
    struct Todo {
        user_id: String,
        order: i64,
        title: String,
    }

    let mut db = test.setup_db(models!(Todo)).await;

    for i in 0..5 {
        Todo::create()
            .user_id("alice")
            .order(i)
            .title(format!("todo-{i}"))
            .exec(&mut db)
            .await?;
    }

    test.log().clear();

    let todo_table_id = table_id(&db, "todos");
    let is_sql = test.capability().sql;

    // Query with ascending order — on NoSQL, order must be Asc.
    let todos: Vec<_> = Todo::filter_by_user_id("alice")
        .order_by(Todo::fields().order().asc())
        .limit(3)
        .collect(&mut db)
        .await?;

    assert_eq!(todos.len(), 3);
    assert!(todos[0].order < todos[1].order);
    assert!(todos[1].order < todos[2].order);

    let (op, _resp) = test.log().pop();

    if is_sql {
        assert_struct!(op, Operation::QuerySql(_ { .. }));
    } else {
        assert_struct!(op, Operation::QueryPk(_ {
            table: == todo_table_id,
            limit: Some(3),
            order: Some(stmt::Direction::Asc),
            ..
        }));
    }

    Ok(())
}
