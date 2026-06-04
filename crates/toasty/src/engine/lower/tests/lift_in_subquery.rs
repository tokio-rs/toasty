//! Tests for `lower::lift_in_subquery::lift_in_subquery`.
//!
//! Migrated from `simplify::tests::lift_in_subquery` when the rewrite
//! moved out of simplify and into the lowering walk.

use crate as toasty;
use crate::engine::lower::lift_in_subquery::lift_in_subquery;
use crate::schema::Model;
use toasty_core::{
    driver::Capability,
    schema::{
        Builder, app,
        app::{FieldId, ModelId},
    },
    stmt::{
        self, Expr, ExprBinaryOp, ExprContext, ExprInSubquery, Projection, Query, Returning, Value,
    },
};

#[allow(dead_code)]
#[derive(toasty::Model)]
struct User {
    #[key]
    id: i64,

    #[has_many]
    posts: toasty::Deferred<Vec<Post>>,
}

#[allow(dead_code)]
#[derive(toasty::Model)]
struct Post {
    #[key]
    id: i64,

    #[index]
    user_id: i64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::Deferred<User>,
}

/// Schema with `User` and `Post` models in a `HasMany`/`BelongsTo`
/// relationship.
struct UserPostSchema {
    schema: toasty_core::Schema,
    user_model: ModelId,
    user_id: FieldId,
    user_posts: FieldId,
    post_model: ModelId,
    post_id: FieldId,
    post_user: FieldId,
    post_user_id: FieldId,
}

impl UserPostSchema {
    fn new() -> Self {
        let app_schema = app::Schema::from_macro([User::schema(), Post::schema()])
            .expect("schema should build from macro");

        let schema = Builder::new()
            .build(app_schema, &Capability::SQLITE)
            .expect("schema should build");

        let user_model = User::id();
        let post_model = Post::id();

        let field = |model, name| {
            schema
                .app
                .model(model)
                .as_root_unwrap()
                .field_by_name(name)
                .unwrap()
                .id
        };

        Self {
            user_id: field(user_model, "id"),
            user_posts: field(user_model, "posts"),
            post_id: field(post_model, "id"),
            post_user: field(post_model, "user"),
            post_user_id: field(post_model, "user_id"),
            schema,
            user_model,
            post_model,
        }
    }
}

#[test]
fn belongs_to_lifts_fk_constraint_to_direct_eq() {
    let s = UserPostSchema::new();
    let cx = ExprContext::new(&s.schema);

    let post_source: stmt::Source = s.post_model.into();
    let scoped_cx = cx.scope(&post_source);

    // `lift_in_subquery(user, select(User, eq(id, 42))) → eq(user_id, 42)`
    let expr = Expr::ref_self_field(s.post_user);
    let filter = Expr::eq(
        Expr::ref_self_field(s.user_id),
        Expr::Value(Value::from(42i64)),
    );
    let query = Query::new_select(s.user_model, filter);

    let result = lift_in_subquery(&scoped_cx, &expr, &query);

    assert!(result.is_some());
    let lifted = result.unwrap();
    let Expr::BinaryOp(ExprBinaryOp { op, lhs, rhs }) = lifted else {
        panic!("expected result to be an `Expr::BinaryOp`");
    };
    assert!(op.is_eq());
    assert!(matches!(
        *lhs,
        Expr::Reference(stmt::ExprReference::Field { index: 1, .. })
    ));
    assert!(matches!(*rhs, Expr::Value(Value::I64(42))));
}

/// `Project(Ref(BelongsTo), [HasIdx])` paired across a shared PK fuses into a
/// single `outer_fk IN (SELECT inner_fk FROM target WHERE …)` — the
/// intermediate routing model (`User` here) is skipped.
///
/// Input AST for `Post.user.posts IN (SELECT FROM Post WHERE id == 42)`:
///   - LHS: `Project(Ref(Post.user), [User.posts_idx])`
///   - RHS: `SELECT FROM Post WHERE Post.id == 42`
///
/// Expected fused output:
///   `Post.user_id IN (SELECT Post.user_id FROM Post WHERE Post.id == 42)`.
#[test]
fn belongs_to_has_many_fuses_to_direct_fk() {
    let s = UserPostSchema::new();
    let cx = ExprContext::new(&s.schema);
    let post_source: stmt::Source = s.post_model.into();
    let scoped_cx = cx.scope(&post_source);

    let expr = Expr::project(
        Expr::ref_self_field(s.post_user),
        Projection::single(s.user_posts.index),
    );
    let inner_filter = Expr::eq(
        Expr::ref_self_field(s.post_id),
        Expr::Value(Value::from(42i64)),
    );
    let query = Query::new_select(s.post_model, inner_filter);

    let lifted = lift_in_subquery(&scoped_cx, &expr, &query).expect("lift to succeed");

    let Expr::InSubquery(ExprInSubquery {
        expr: lhs,
        query: fused,
    }) = lifted
    else {
        panic!("expected InSubquery, got {lifted:?}");
    };

    // Outer LHS is the FK column on Post (no projection through a relation).
    assert!(matches!(
        *lhs,
        Expr::Reference(stmt::ExprReference::Field { nesting: 0, index })
            if index == s.post_user_id.index
    ));

    let select = fused.body.as_select_unwrap();

    // No routing through User: the fused subquery scans Post directly.
    assert_eq!(select.source.model_id_unwrap(), s.post_model);

    // Projects the FK column (Post.user_id), not Post's PK.
    let Returning::Project(returning) = &select.returning else {
        panic!("expected Returning::Project, got {:?}", select.returning);
    };
    let Expr::Reference(stmt::ExprReference::Field {
        index: ret_index, ..
    }) = returning
    else {
        panic!("expected single-column field projection, got {returning:?}");
    };
    assert_eq!(*ret_index, s.post_user_id.index);

    // The inner filter passes through unchanged: `Post.id == 42`.
    let Some(Expr::BinaryOp(ExprBinaryOp { op, lhs, rhs })) = &select.filter.expr else {
        panic!("expected BinaryOp filter, got {:?}", select.filter.expr);
    };
    assert!(op.is_eq());
    assert!(matches!(
        **lhs,
        Expr::Reference(stmt::ExprReference::Field { index, .. })
            if index == s.post_id.index
    ));
    assert!(matches!(**rhs, Expr::Value(Value::I64(42))));
}

/// `Project` with a non-relation base bails (no lift, no panic) — the lift is
/// only meaningful when the path passes through a relation field.
#[test]
fn project_with_non_relation_base_returns_none() {
    let s = UserPostSchema::new();
    let cx = ExprContext::new(&s.schema);
    let post_source: stmt::Source = s.post_model.into();
    let scoped_cx = cx.scope(&post_source);

    // `Project(Ref(Post.id), [0])` — projecting through a scalar PK field, not
    // a relation. The lift can't re-root this; it must return None.
    let expr = Expr::project(Expr::ref_self_field(s.post_id), Projection::single(0));
    let query = Query::new_select(
        s.post_model,
        Expr::eq(
            Expr::ref_self_field(s.post_id),
            Expr::Value(Value::from(1i64)),
        ),
    );

    assert!(lift_in_subquery(&scoped_cx, &expr, &query).is_none());
}
