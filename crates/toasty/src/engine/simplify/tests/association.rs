use crate as toasty;
use crate::engine::simplify::Simplify;
use crate::schema::Register;
use toasty_core::{
    driver::Capability,
    schema::{app, app::FieldId, app::ModelId, Builder},
    stmt::{self, Association, Expr, ExprInSubquery, Path, Query, SourceModel, Value},
};

#[allow(dead_code)]
#[derive(toasty::Model)]
struct User {
    #[key]
    id: i64,

    #[has_many(pair = author)]
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
    author: toasty::BelongsTo<User>,
}

#[allow(dead_code)]
#[derive(toasty::Model)]
struct Account {
    #[key]
    id: i64,

    #[has_one]
    profile: toasty::HasOne<Option<Profile>>,
}

#[allow(dead_code)]
#[derive(toasty::Model)]
struct Profile {
    #[key]
    id: i64,

    #[unique]
    account_id: i64,

    #[belongs_to(key = account_id, references = id)]
    account: toasty::BelongsTo<Account>,
}

struct UserPostSchema {
    schema: toasty_core::Schema,
    user_model: ModelId,
    user_id: FieldId,
    user_posts: FieldId,
    post_model: ModelId,
    post_user_id: FieldId,
    post_author: FieldId,
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
            .as_root_unwrap()
            .field_by_name("id")
            .unwrap()
            .id;
        let user_posts = schema
            .app
            .model(user_model)
            .as_root_unwrap()
            .field_by_name("posts")
            .unwrap()
            .id;
        let post_user_id = schema
            .app
            .model(post_model)
            .as_root_unwrap()
            .field_by_name("user_id")
            .unwrap()
            .id;
        let post_author = schema
            .app
            .model(post_model)
            .as_root_unwrap()
            .field_by_name("author")
            .unwrap()
            .id;

        Self {
            schema,
            user_model,
            user_id,
            user_posts,
            post_model,
            post_user_id,
            post_author,
        }
    }
}

#[test]
fn has_many_via_becomes_in_subquery() {
    // `select(Post, via(User.posts)) → select(Post, in_subquery(author, user_query))`
    let s = UserPostSchema::new();
    let mut simplify = Simplify::new(&s.schema);

    let user_filter = Expr::eq(
        Expr::ref_self_field(s.user_id),
        Expr::Value(Value::from(42i64)),
    );
    let user_query = Query::new_select(s.user_model, user_filter);

    let association = Association {
        source: Box::new(user_query),
        path: Path::field(s.user_model, s.user_posts.index),
    };

    let mut query = Query::new_select(s.post_model, Expr::Value(Value::Bool(true)));
    if let stmt::ExprSet::Select(select) = &mut query.body {
        if let stmt::Source::Model(model) = &mut select.source {
            model.via = Some(association);
        }
    }

    simplify.simplify_via_association_for_query(&mut query);

    let stmt::ExprSet::Select(select) = &query.body else {
        panic!("expected Select");
    };
    assert!(matches!(
        &select.source,
        stmt::Source::Model(SourceModel { via: None, .. })
    ));

    let filter_expr = select.filter.as_expr();
    let Expr::InSubquery(ExprInSubquery {
        expr,
        query: subquery,
    }) = filter_expr
    else {
        panic!("expected filter expression to be an `Expr::InSubquery`");
    };

    // The expression should reference the pair field (`post_author`).
    assert!(matches!(
        &**expr,
        Expr::Reference(stmt::ExprReference::Field { index, .. }) if *index == s.post_author.index
    ));

    // The subquery should be the user query.
    let stmt::ExprSet::Select(select) = &subquery.body else {
        panic!("expected subquery body to be a `ExprSet::Select`");
    };

    // Ensure the source of the subquery is the user model.
    assert!(matches!(
        &select.source,
        stmt::Source::Model(SourceModel { id, .. }) if *id == s.user_model
    ));
}

#[test]
fn has_many_via_references_fk_field_not_relation_field() {
    // The `in_subquery` LHS should reference the FK column field (`post.user_id`),
    // not the BelongsTo relation field (`post.author`). Using the relation field
    // is incorrect because it doesn't map to a column during lowering.
    let s = UserPostSchema::new();
    let mut simplify = Simplify::new(&s.schema);

    let user_filter = Expr::eq(
        Expr::ref_self_field(s.user_id),
        Expr::Value(Value::from(42i64)),
    );
    let user_query = Query::new_select(s.user_model, user_filter);

    let association = Association {
        source: Box::new(user_query),
        path: Path::field(s.user_model, s.user_posts.index),
    };

    let mut query = Query::new_select(s.post_model, Expr::Value(Value::Bool(true)));
    if let stmt::ExprSet::Select(select) = &mut query.body {
        if let stmt::Source::Model(model) = &mut select.source {
            model.via = Some(association);
        }
    }

    simplify.simplify_via_association_for_query(&mut query);

    let stmt::ExprSet::Select(select) = &query.body else {
        panic!("expected Select");
    };

    let filter_expr = select.filter.as_expr();
    let Expr::InSubquery(ExprInSubquery { expr, .. }) = filter_expr else {
        panic!("expected filter to be InSubquery");
    };

    // Should reference user_id (the FK column field), not author (the relation field)
    assert!(
        matches!(
            &**expr,
            Expr::Reference(stmt::ExprReference::Field { index, .. }) if *index == s.post_user_id.index
        ),
        "expected InSubquery LHS to reference FK field `user_id` (index {}), \
         but got: {expr:?}",
        s.post_user_id.index,
    );
}

#[test]
fn has_many_via_sets_subquery_returning_to_pk() {
    // The subquery in the `in_subquery` expression should have its `returning`
    // clause set to return only the PK field that the FK references (`user.id`),
    // not the full model. Without this, the subquery returns all columns which
    // is incorrect for an IN expression.
    let s = UserPostSchema::new();
    let mut simplify = Simplify::new(&s.schema);

    let user_filter = Expr::eq(
        Expr::ref_self_field(s.user_id),
        Expr::Value(Value::from(42i64)),
    );
    let user_query = Query::new_select(s.user_model, user_filter);

    let association = Association {
        source: Box::new(user_query),
        path: Path::field(s.user_model, s.user_posts.index),
    };

    let mut query = Query::new_select(s.post_model, Expr::Value(Value::Bool(true)));
    if let stmt::ExprSet::Select(select) = &mut query.body {
        if let stmt::Source::Model(model) = &mut select.source {
            model.via = Some(association);
        }
    }

    simplify.simplify_via_association_for_query(&mut query);

    let stmt::ExprSet::Select(select) = &query.body else {
        panic!("expected Select");
    };

    let filter_expr = select.filter.as_expr();
    let Expr::InSubquery(ExprInSubquery {
        query: subquery, ..
    }) = filter_expr
    else {
        panic!("expected filter to be InSubquery");
    };

    let sub_returning = subquery.returning_unwrap();

    // The subquery should return only the user.id field (FK target), not Model
    assert!(
        matches!(sub_returning, stmt::Returning::Expr(_)),
        "expected subquery returning to be Expr (projecting the PK field), \
         but got: {sub_returning:?}",
    );
}

#[test]
fn has_one_via_references_fk_field_not_relation_field() {
    // Same issue as HasMany: the `in_subquery` LHS should reference the FK
    // column field (`profile.account_id`), not the BelongsTo relation field.
    let app_schema = app::Schema::from_macro(&[Account::schema(), Profile::schema()])
        .expect("schema should build from macro");

    let schema = Builder::new()
        .build(app_schema, &Capability::SQLITE)
        .expect("schema should build");

    let account_model = Account::id();
    let profile_model = Profile::id();

    let account_id = schema
        .app
        .model(account_model)
        .as_root_unwrap()
        .field_by_name("id")
        .unwrap()
        .id;
    let account_profile = schema
        .app
        .model(account_model)
        .as_root_unwrap()
        .field_by_name("profile")
        .unwrap()
        .id;
    let profile_account_id = schema
        .app
        .model(profile_model)
        .as_root_unwrap()
        .field_by_name("account_id")
        .unwrap()
        .id;

    let mut simplify = Simplify::new(&schema);

    let account_filter = Expr::eq(
        Expr::ref_self_field(account_id),
        Expr::Value(Value::from(99i64)),
    );
    let account_query = Query::new_select(account_model, account_filter);

    let association = Association {
        source: Box::new(account_query),
        path: Path::field(account_model, account_profile.index),
    };

    let mut query = Query::new_select(profile_model, Expr::Value(Value::Bool(true)));
    if let stmt::ExprSet::Select(select) = &mut query.body {
        if let stmt::Source::Model(model) = &mut select.source {
            model.via = Some(association);
        }
    }

    simplify.simplify_via_association_for_query(&mut query);

    let stmt::ExprSet::Select(select) = &query.body else {
        panic!("expected Select");
    };

    let filter_expr = select.filter.as_expr();
    let Expr::InSubquery(ExprInSubquery {
        expr,
        query: subquery,
        ..
    }) = filter_expr
    else {
        panic!("expected filter to be InSubquery");
    };

    // Should reference account_id (the FK column field), not account (the relation field)
    assert!(
        matches!(
            &**expr,
            Expr::Reference(stmt::ExprReference::Field { index, .. }) if *index == profile_account_id.index
        ),
        "expected InSubquery LHS to reference FK field `account_id` (index {}), \
         but got: {expr:?}",
        profile_account_id.index,
    );

    // The subquery should also set returning to the PK field
    let sub_returning = subquery.returning_unwrap();
    assert!(
        matches!(sub_returning, stmt::Returning::Expr(_)),
        "expected subquery returning to be Expr (projecting the PK field), \
         but got: {sub_returning:?}",
    );
}
