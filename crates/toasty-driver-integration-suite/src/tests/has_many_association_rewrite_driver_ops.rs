//! Tests verifying that HasMany/HasOne association rewriting produces correct
//! lowered statements.
//!
//! The `rewrite_association_as_filter` simplification for HasMany/HasOne
//! currently passes `ref_self_field(rel.pair)` (the BelongsTo relation field)
//! and doesn't set the subquery's `returning` clause. The downstream
//! `lift_in_subquery` optimization compensates for this in the normal
//! association flow. These tests bypass that compensation to expose the
//! underlying bug by using primitive FK fields (which `lift_in_subquery`
//! ignores) combined with `Returning::Model` on the subquery.

use crate::prelude::*;

use toasty::Executor;
use toasty_core::stmt::{self, Expr, Value};

/// Submitting a hand-crafted `InSubquery` where the LHS is a primitive FK
/// field (so `lift_in_subquery` won't rewrite it) and the subquery has
/// `Returning::Model` (the buggy form that HasMany/HasOne produces).
///
/// The subquery should return a single column (the PK that the FK references),
/// but `Returning::Model` causes it to return ALL columns after lowering.
///
/// For id_u64: SQLite rejects with "sub-select returns 3 columns - expected 1"
/// For id_uuid: lowering panics in `cast_expr` trying to cast a multi-column
///              record to a scalar type
#[driver_test(id(ID), requires(sql))]
#[should_panic]
pub async fn has_many_in_subquery_with_returning_model_fails(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    // Create test data
    let alice = User::create().name("Alice").exec(&mut db).await?;
    alice
        .todos()
        .create()
        .title("buy groceries")
        .exec(&mut db)
        .await?;

    // Construct a raw statement that mimics the buggy output of
    // `rewrite_association_as_filter` for HasMany/HasOne:
    //
    //   SELECT * FROM Todo
    //   WHERE user_id IN (SELECT <all model columns> FROM User WHERE id = ?)
    //
    // The `user_id` LHS is a primitive FK field, so `lift_in_subquery` won't
    // rewrite it (it only rewrites relation field references). The subquery
    // has `Returning::Model` (what `new_select` produces by default) instead
    // of `Returning::Expr(ref_self_field(user.id))` — this is the actual bug
    // in the HasMany/HasOne branches of `rewrite_association_as_filter`.
    let schema = db.schema();
    let user_model_id = User::id();
    let todo_model_id = Todo::id();

    let todo_user_id_field = schema
        .app
        .model(todo_model_id)
        .as_root_unwrap()
        .field_by_name("user_id")
        .unwrap();
    let user_id_field = schema
        .app
        .model(user_model_id)
        .as_root_unwrap()
        .field_by_name("id")
        .unwrap();

    let user_filter = Expr::eq(
        Expr::ref_self_field(user_id_field.id),
        Expr::Value(Value::from(alice.id)),
    );
    let user_subquery = stmt::Query::new_select(user_model_id, user_filter);
    // ^^^ Returning::Model { include: [] } — the bug: should be
    // Returning::Expr(ref_self_field(user.id))

    let in_subquery_filter =
        Expr::in_subquery(Expr::ref_self_field(todo_user_id_field.id), user_subquery);

    let todo_query = stmt::Query::new_select(todo_model_id, in_subquery_filter);
    let raw_stmt = stmt::Statement::Query(todo_query);

    // This panics because the IN subquery returns multiple columns
    // (Returning::Model is lowered to all model columns) instead of a single
    // PK column. The fix is for rewrite_association_as_filter to set
    // Returning::Expr on the subquery, like the BelongsTo branch does.
    let mut stream = db.exec_untyped(raw_stmt).await?;
    while let Some(value) = stream.next().await {
        let _ = value?;
    }

    Ok(())
}
