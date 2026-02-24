use crate as toasty;
use crate::engine::simplify::Simplify;
use crate::model::Register;
use toasty_core::{
    driver::Capability,
    schema::{
        app,
        app::{FieldId, ModelId},
        Builder,
    },
    stmt::{self, Expr, ExprBinaryOp, Query, Value},
};

#[allow(dead_code)]
#[derive(toasty::Model)]
struct User {
    #[key]
    id: i64,

    #[has_many]
    posts: toasty::HasMany<Post>,
}

#[allow(dead_code)]
#[derive(toasty::Model)]
struct Post {
    #[key]
    id: i64,

    #[index]
    user_id: i64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,
}

/// Schema with `User` and `Post` models in a `HasMany`/`BelongsTo`
/// relationship.
struct UserPostSchema {
    schema: toasty_core::Schema,
    user_model: ModelId,
    user_id: FieldId,
    post_model: ModelId,
    post_user: FieldId,
}

impl UserPostSchema {
    fn new() -> Self {
        let app_schema = app::Schema::from_macro(&[User::schema(), Post::schema()])
            .expect("schema should build from macro");

        let schema = Builder::new()
            .build(app_schema, &Capability::SQLITE)
            .expect("schema should build");

        let user_model = User::id();
        let post_model = Post::id();

        // Find field IDs by name from the generated schema
        let user_id = schema
            .app
            .model(user_model)
            .expect_root()
            .field_by_name("id")
            .unwrap()
            .id;
        let post_user = schema
            .app
            .model(post_model)
            .expect_root()
            .field_by_name("user")
            .unwrap()
            .id;

        Self {
            schema,
            user_model,
            user_id,
            post_model,
            post_user,
        }
    }
}

#[test]
fn belongs_to_lifts_fk_constraint_to_direct_eq() {
    let s = UserPostSchema::new();
    let simplify = Simplify::new(&s.schema);

    let post_source: stmt::Source = s.post_model.into();
    let mut scoped_simplify = simplify.scope(&post_source);

    // `lift_in_subquery(user, select(User, eq(id, 42))) → eq(user_id, 42)`
    let expr = Expr::ref_self_field(s.post_user);
    let filter = Expr::eq(
        Expr::ref_self_field(s.user_id),
        Expr::Value(Value::from(42i64)),
    );
    let query = Query::new_select(s.user_model, filter);

    let result = scoped_simplify.lift_in_subquery(&expr, &query);

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
