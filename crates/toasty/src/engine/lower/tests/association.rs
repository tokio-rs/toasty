use crate as toasty;
use crate::engine::lower::association::RewriteVia;
use crate::schema::Register;
use toasty_core::{
    driver::Capability,
    schema::{Builder, app, app::FieldId, app::ModelId},
    stmt::{
        self, Association, Expr, ExprContext, ExprInSubquery, Path, Projection, Query, Returning,
        SourceModel, Value,
    },
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
        let app_schema = app::Schema::from_macro([User::schema(), Post::schema()])
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
    let cx = ExprContext::new(&s.schema);
    let mut rewrite = RewriteVia::new(cx);

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
    if let stmt::ExprSet::Select(select) = &mut query.body
        && let stmt::Source::Model(model) = &mut select.source
    {
        model.via = Some(association);
    }

    rewrite.rewrite_via_for_query(&mut query);

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
fn multi_step_via_unfolds_into_nested_in_subqueries() {
    // `select(User, via(User.posts, Post.author))` should unfold into:
    //     User WHERE id IN (SELECT post.user_id FROM Post WHERE author IN user_query)
    let s = UserPostSchema::new();
    let cx = ExprContext::new(&s.schema);
    let mut rewrite = RewriteVia::new(cx);

    let user_filter = Expr::eq(
        Expr::ref_self_field(s.user_id),
        Expr::Value(Value::from(42i64)),
    );
    let user_query = Query::new_select(s.user_model, user_filter);

    let mut path = Path::model(s.user_model);
    path.projection = Projection::single(s.user_posts.index);
    path.projection.push(s.post_author.index);

    let association = Association {
        source: Box::new(user_query),
        path,
    };

    let mut query = Query::new_select(s.user_model, Expr::Value(Value::Bool(true)));
    if let stmt::ExprSet::Select(select) = &mut query.body
        && let stmt::Source::Model(model) = &mut select.source
    {
        model.via = Some(association);
    }

    rewrite.rewrite_via_for_query(&mut query);

    let stmt::ExprSet::Select(select) = &query.body else {
        panic!("expected outer Select");
    };
    assert!(matches!(
        &select.source,
        stmt::Source::Model(SourceModel { id, via: None }) if *id == s.user_model
    ));

    // Outer filter: `User.id IN (SELECT post.user_id FROM Post WHERE …)`
    let Expr::InSubquery(ExprInSubquery {
        expr,
        query: outer_subquery,
    }) = select.filter.as_expr()
    else {
        panic!("expected outer filter to be Expr::InSubquery");
    };
    assert!(matches!(
        &**expr,
        Expr::Reference(stmt::ExprReference::Field { index, .. }) if *index == s.user_id.index
    ));

    // The outer subquery is over Post, projects post.user_id, and has its own
    // `author IN user_query` filter.
    let stmt::ExprSet::Select(post_select) = &outer_subquery.body else {
        panic!("expected outer subquery body to be Select");
    };
    assert!(matches!(
        &post_select.source,
        stmt::Source::Model(SourceModel { id, via: None }) if *id == s.post_model
    ));

    let Returning::Project(project) = &post_select.returning else {
        panic!("expected post subquery to project its returning");
    };
    assert!(matches!(
        project,
        Expr::Reference(stmt::ExprReference::Field { index, .. }) if *index == s.post_user_id.index
    ));

    // The post subquery's filter must contain the inner `author IN user_query`
    // produced by the recursive single-step rewrite.
    let post_filter = post_select.filter.as_expr();
    let inner_in_subquery = find_in_subquery(post_filter)
        .expect("expected inner Expr::InSubquery inside post subquery filter");
    assert!(matches!(
        &*inner_in_subquery.expr,
        Expr::Reference(stmt::ExprReference::Field { index, .. }) if *index == s.post_author.index
    ));
    let stmt::ExprSet::Select(user_select) = &inner_in_subquery.query.body else {
        panic!("expected inner subquery body to be Select");
    };
    assert!(matches!(
        &user_select.source,
        stmt::Source::Model(SourceModel { id, via: None }) if *id == s.user_model
    ));
}

fn find_in_subquery(expr: &Expr) -> Option<&ExprInSubquery> {
    match expr {
        Expr::InSubquery(in_sub) => Some(in_sub),
        Expr::BinaryOp(b) => find_in_subquery(&b.lhs).or_else(|| find_in_subquery(&b.rhs)),
        Expr::And(and) => and.operands.iter().find_map(find_in_subquery),
        Expr::Or(or) => or.operands.iter().find_map(find_in_subquery),
        _ => None,
    }
}
