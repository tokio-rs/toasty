use crate::prelude::*;

use toasty_core::{
    driver::{Operation, Rows},
    stmt::{Source, Statement, UpdateTarget},
};

/// Test update on a model with a partitioned composite primary key using the
/// partition-key-only filter.
///
/// `Todo::filter_by_user_id(user_id).update()` uses only the partition key in the
/// filter expression. For NoSQL (DynamoDB), this requires a `QueryPk` to find all
/// matching records and then an `UpdateItem` for each — not just a bare `QueryPk`
/// that silently discards the mutation.
#[driver_test]
pub async fn update_by_partition_key(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[key(partition = user_id, local = id)]
    struct Todo {
        #[auto]
        id: uuid::Uuid,

        user_id: String,

        title: String,
    }

    let db = test.setup_db(models!(Todo)).await;

    let todo_table_id = table_id(&db, "todos");
    let is_sql = test.capability().sql;

    let todo1 = Todo::create()
        .user_id("alice")
        .title("original1")
        .exec(&db)
        .await
        .unwrap();

    let todo2 = Todo::create()
        .user_id("alice")
        .title("original2")
        .exec(&db)
        .await
        .unwrap();

    test.log().clear();

    // Update all todos for "alice" using only the partition key filter.
    Todo::filter_by_user_id("alice")
        .update()
        .title("updated")
        .exec(&db)
        .await
        .unwrap();

    if is_sql {
        let (op, resp) = test.log().pop();

        // Column index 2 = title (id=0, user_id=1, title=2).
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Update(_ {
                target: UpdateTarget::Table(== todo_table_id),
                assignments: #{ 2: _ { expr: == "updated", .. }},
                ..
            }),
            ret: None,
            ..
        }));

        assert_struct!(resp, _ {
            rows: Rows::Count(_),
            ..
        });
    } else {
        // NoSQL: first a QueryPk to collect all matching PKs, then UpdateByKey
        // for every matched record.
        let (op, _) = test.log().pop();

        assert_struct!(op, Operation::QueryPk(_ {
            table: == todo_table_id,
            select.len(): 2,
            filter: None,
            ..
        }));

        let (op, resp) = test.log().pop();

        // Column index 2 = title (id=0, user_id=1, title=2).
        assert_struct!(op, Operation::UpdateByKey(_ {
            table: == todo_table_id,
            keys.len(): 2,
            assignments: #{ 2: _ { expr: == "updated", .. }},
            filter: None,
            returning: false,
            ..
        }));

        assert_struct!(resp, _ {
            rows: Rows::Count(2),
            ..
        });
    }

    assert!(test.log().is_empty(), "log should be empty after update");

    test.log().clear();
    let reloaded1 = Todo::get_by_user_id_and_id(&db, &todo1.user_id, todo1.id)
        .await
        .unwrap();
    assert_eq!(reloaded1.title, "updated");

    test.log().clear();
    let reloaded2 = Todo::get_by_user_id_and_id(&db, &todo2.user_id, todo2.id)
        .await
        .unwrap();
    assert_eq!(reloaded2.title, "updated");
}

/// Test delete on a model with a partitioned composite primary key using the
/// partition-key-only filter.
///
/// `Todo::filter_by_user_id(user_id).delete()` must delete all matching records,
/// not silently skip the deletion by issuing only a read-only `QueryPk`.
#[driver_test]
pub async fn delete_by_partition_key(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[key(partition = user_id, local = id)]
    struct Todo {
        #[auto]
        id: uuid::Uuid,

        user_id: String,

        title: String,
    }

    let db = test.setup_db(models!(Todo)).await;

    let todo_table_id = table_id(&db, "todos");
    let is_sql = test.capability().sql;

    let todo1 = Todo::create()
        .user_id("alice")
        .title("todo1")
        .exec(&db)
        .await
        .unwrap();

    let todo2 = Todo::create()
        .user_id("alice")
        .title("todo2")
        .exec(&db)
        .await
        .unwrap();

    let user_id = todo1.user_id.clone();
    let id1 = todo1.id;
    let id2 = todo2.id;

    test.log().clear();

    // Delete all todos for "alice" using only the partition key filter.
    Todo::filter_by_user_id("alice").delete(&db).await.unwrap();

    if is_sql {
        let (op, resp) = test.log().pop();

        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Delete(_ {
                from: Source::Table(_ {
                    tables: [== todo_table_id, ..],
                    ..
                }),
                ..
            }),
            ..
        }));

        assert_struct!(resp, _ {
            rows: Rows::Count(_),
            ..
        });
    } else {
        // NoSQL: first a QueryPk to collect all matching PKs, then DeleteByKey
        // for every matched record.
        let (op, _) = test.log().pop();

        assert_struct!(op, Operation::QueryPk(_ {
            table: == todo_table_id,
            select.len(): 2,
            filter: None,
            ..
        }));

        let (op, resp) = test.log().pop();

        assert_struct!(op, Operation::DeleteByKey(_ {
            table: == todo_table_id,
            keys.len(): 2,
            filter: None,
            ..
        }));

        assert_struct!(resp, _ {
            rows: Rows::Count(2),
            ..
        });
    }

    assert!(test.log().is_empty(), "log should be empty after delete");

    assert_err!(Todo::get_by_user_id_and_id(&db, &user_id, id1).await);
    assert_err!(Todo::get_by_user_id_and_id(&db, &user_id, id2).await);
}
