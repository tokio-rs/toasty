use super::test_schema_with;
use crate as toasty;
use crate::{engine::simplify::Simplify, schema::Model};
use toasty_core::{
    driver::Capability,
    schema::app::ModelId,
    stmt::{self, Expr, ExprContext, Query, Returning, VisitMut},
};

#[allow(dead_code)]
#[derive(toasty::Model)]
struct User {
    #[key]
    id: i64,
    name: String,
    active: bool,
    #[unique]
    email: String,
    #[unique]
    nullable_email: Option<String>,
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
    #[belongs_to(key = user_id)]
    user: toasty::Deferred<User>,
    optional_key: Option<i64>,
    other_key: i64,
}

#[allow(dead_code)]
#[derive(toasty::Model)]
#[key(partition = tenant, local = id)]
struct Item {
    tenant: String,
    id: i64,
    value: String,
}

fn schema() -> toasty_core::Schema {
    test_schema_with(&[User::schema(), Post::schema(), Item::schema()])
}

fn field<M: Model>(index: usize) -> Expr {
    Expr::ref_self_field(M::id().field(index))
}

fn query(model: ModelId, projection: Expr, filter: Expr) -> Query {
    let mut query = Query::new_select(model, filter);
    query.body.as_select_mut_unwrap().returning = Returning::Project(projection);
    query
}

fn users(filter: Expr) -> Query {
    query(User::id(), field::<User>(0), filter)
}

fn membership(filter: Expr) -> Expr {
    Expr::in_subquery(field::<Post>(1), users(filter))
}

fn name() -> Expr {
    Expr::eq(field::<User>(1), "alice")
}

fn active() -> Expr {
    field::<User>(2)
}

fn simplify_filter(expr: Expr) -> Expr {
    let schema = schema();
    let mut query = Query::new_select(Post::id(), expr);
    Simplify::new(&schema, &Capability::SQLITE).visit_stmt_query_mut(&mut query);
    query.body.as_select_mut_unwrap().filter.take().into_expr()
}

#[test]
fn combines_explicit_subqueries() {
    assert_eq!(
        simplify_filter(Expr::and(membership(name()), membership(active()))),
        membership(Expr::and(name(), active()))
    );
}

#[test]
fn combines_nonadjacent_operands_and_multiple_groups() {
    let other = |filter| Expr::in_subquery(field::<Post>(4), users(filter));
    let unrelated = Expr::eq(field::<Post>(0), 42);
    let input = stmt::ExprAnd {
        operands: vec![
            membership(name()),
            other(name()),
            unrelated.clone(),
            membership(active()),
            other(active()),
            membership(Expr::eq(field::<User>(3), "a@b")),
        ],
    }
    .into();
    assert_eq!(
        simplify_filter(input),
        Expr::from(stmt::ExprAnd {
            operands: vec![
                membership(Expr::and(
                    Expr::and(name(), active()),
                    Expr::eq(field::<User>(3), "a@b")
                )),
                other(Expr::and(name(), active())),
                unrelated
            ],
        })
    );
}

#[test]
fn combines_compound_filters() {
    let compound = Expr::or(name(), active());
    let email = Expr::eq(field::<User>(3), "a@b");
    assert_eq!(
        simplify_filter(Expr::and(membership(compound), membership(email.clone()))),
        membership(Expr::and(email, Expr::or(active(), name())))
    );
}

#[test]
fn combines_filters_containing_nested_subqueries() {
    let nested = Expr::in_subquery(
        field::<User>(0),
        query(Post::id(), field::<Post>(1), Expr::eq(field::<Post>(0), 42)),
    );
    assert_eq!(
        simplify_filter(Expr::and(membership(name()), membership(nested.clone()))),
        membership(Expr::and(name(), nested))
    );
}

#[test]
fn simplifies_newly_combined_filters_recursively() {
    let nested = |filter| Expr::in_subquery(field::<User>(0), users(filter));
    assert_eq!(
        simplify_filter(Expr::and(
            membership(nested(name())),
            membership(nested(active()))
        )),
        membership(nested(Expr::and(name(), active())))
    );
}

#[test]
fn combines_unique_secondary_key() {
    let by_email = |filter| Expr::in_subquery("a@b", query(User::id(), field::<User>(3), filter));
    assert_eq!(
        simplify_filter(Expr::and(by_email(name()), by_email(active()))),
        by_email(Expr::and(name(), active()))
    );
}

#[test]
fn combines_complete_composite_key_but_not_partial_key() {
    let a = Expr::eq(field::<Item>(2), "a");
    let b = Expr::ne(field::<Item>(2), "b");
    let key = Expr::record([field::<Item>(0), field::<Item>(1)]);
    let by_key = |filter| {
        Expr::in_subquery(
            Expr::Value(stmt::Value::Record(stmt::ValueRecord::from_vec(vec![
                "tenant".into(),
                1.into(),
            ]))),
            query(Item::id(), key.clone(), filter),
        )
    };
    assert_eq!(
        simplify_filter(Expr::and(by_key(a.clone()), by_key(b.clone()))),
        by_key(Expr::and(a.clone(), b.clone()))
    );

    let partial = |filter| Expr::in_subquery(1, query(Item::id(), field::<Item>(1), filter));
    let input = Expr::and(partial(a), partial(b));
    assert_eq!(simplify_filter(input.clone()), input);
}

#[test]
fn preserves_nonunique_and_nullable_projections() {
    for key in [field::<User>(1), field::<User>(4)] {
        let by_key = |filter| Expr::in_subquery("alice", query(User::id(), key.clone(), filter));
        let input = Expr::and(by_key(name()), by_key(active()));
        assert_eq!(simplify_filter(input.clone()), input);
    }
}

#[test]
fn preserves_has_many_memberships() {
    let child = |id| {
        Expr::in_subquery(
            field::<User>(0),
            query(Post::id(), field::<Post>(1), Expr::eq(field::<Post>(0), id)),
        )
    };
    let input = Expr::and(child(1), child(2));
    let mut outer = Query::new_select(User::id(), input.clone());
    Simplify::new(&schema(), &Capability::SQLITE).visit_stmt_query_mut(&mut outer);
    assert_eq!(outer.body.as_select_unwrap().filter.as_expr(), &input);
}

#[test]
fn preserves_different_sources_projections_and_operands() {
    let others = [
        Expr::in_subquery(field::<Post>(4), users(active())),
        Expr::in_subquery(
            field::<Post>(1),
            query(Post::id(), field::<Post>(0), Expr::eq(field::<Post>(4), 1)),
        ),
        Expr::in_subquery(
            field::<Post>(1),
            query(User::id(), field::<User>(3), active()),
        ),
    ];
    for other in others {
        let input = Expr::and(membership(name()), other);
        assert_eq!(simplify_filter(input.clone()), input);
    }
}

#[test]
fn preserves_query_modifiers_and_negated_membership() {
    let base = users(active());
    let mut limited = base.clone();
    limited.limit = Some(stmt::Limit::Offset(stmt::LimitOffset {
        limit: 1.into(),
        offset: None,
    }));
    let mut locked = base.clone();
    locked.locks.push(stmt::Lock::Update);
    let mut single = base.clone();
    single.single = true;
    let others = [
        Expr::in_subquery(field::<Post>(1), limited),
        Expr::in_subquery(field::<Post>(1), locked),
        Expr::in_subquery(field::<Post>(1), single),
        Expr::not_in_subquery(field::<Post>(1), base),
    ];
    for other in others {
        let input = Expr::and(membership(name()), other);
        assert_eq!(simplify_filter(input.clone()), input);
    }
}

#[test]
fn preserves_unstable_and_correlated_filters() {
    for filter in [
        Expr::eq(field::<User>(0), Expr::last_insert_id()),
        Expr::eq(field::<User>(0), Expr::ref_field(1, Post::id().field(0))),
    ] {
        let input = Expr::and(membership(name()), membership(filter));
        assert_eq!(simplify_filter(input.clone()), input);
    }
    let unstable = |filter| Expr::in_subquery(Expr::last_insert_id(), users(filter));
    let input = Expr::and(unstable(name()), unstable(active()));
    assert_eq!(simplify_filter(input.clone()), input);
}

#[test]
fn nullable_operand_merges_only_in_positive_filters() {
    let nullable = |filter| Expr::in_subquery(field::<Post>(3), users(filter));
    let input = Expr::and(nullable(name()), nullable(active()));
    assert_eq!(
        simplify_filter(input.clone()),
        nullable(Expr::and(name(), active()))
    );

    let schema = schema();
    let source = stmt::Source::from(Post::id());
    let mut value = input.clone();
    Simplify::with_context(
        ExprContext::new(&schema).scope(&source),
        &Capability::SQLITE,
    )
    .visit_expr_mut(&mut value);
    assert_eq!(value, input);
    assert_eq!(
        simplify_filter(Expr::is_null(input.clone())),
        Expr::is_null(input.clone())
    );
    // Folding distributes NOT; the memberships must still be separate.
    assert_eq!(
        simplify_filter(Expr::not(input)),
        Expr::or(Expr::not(nullable(name())), Expr::not(nullable(active())))
    );
}

#[test]
fn nonnullable_operand_merges_as_a_value() {
    let schema = schema();
    let source = stmt::Source::from(Post::id());
    let mut input = Expr::and(membership(name()), membership(active()));
    Simplify::with_context(
        ExprContext::new(&schema).scope(&source),
        &Capability::SQLITE,
    )
    .visit_expr_mut(&mut input);
    assert_eq!(input, membership(Expr::and(name(), active())));
}

#[test]
fn nosql_nullable_membership_preserves_boolean_value_semantics() {
    let schema = schema();
    let source = stmt::Source::from(Post::id());
    let nullable = |filter| Expr::in_subquery(field::<Post>(3), users(filter));
    let mut input = Expr::and(nullable(name()), nullable(active()));
    Simplify::with_context(
        ExprContext::new(&schema).scope(&source),
        &Capability::DYNAMODB,
    )
    .visit_expr_mut(&mut input);
    assert_eq!(input, nullable(Expr::and(name(), active())));
}

#[test]
fn merges_mixed_relation_path_and_explicit_subquery_before_extraction() {
    let path = Expr::project(field::<Post>(2), [1]);
    let input = Expr::and(Expr::eq(path, "alice"), membership(active()));
    let expected = membership(Expr::and(name(), active()));
    let engine = crate::engine::Engine::new(schema().into(), &Capability::SQLITE);
    let actual = engine
        .lower_stmt(Query::new_select(Post::id(), input).into())
        .unwrap();
    let expected = engine
        .lower_stmt(Query::new_select(Post::id(), expected).into())
        .unwrap();
    assert_eq!(actual.root().stmt(), expected.root().stmt());
}

#[test]
fn combines_before_nosql_dependency_extraction() {
    let capability = &Capability::DYNAMODB;
    let app = toasty_core::schema::app::Schema::from_macro([
        User::schema(),
        Post::schema(),
        Item::schema(),
    ])
    .unwrap();
    let schema = toasty_core::schema::Builder::new()
        .build(app, capability)
        .unwrap();
    let engine = crate::engine::Engine::new(schema.into(), capability);
    let path = Expr::project(field::<Post>(2), [1]);
    let input = Expr::and(Expr::eq(path, "alice"), membership(active()));
    let hir = engine
        .lower_stmt(Query::new_select(Post::id(), input).into())
        .unwrap();
    assert_eq!(hir.statements().count(), 2);
    assert_eq!(hir.root().args.len(), 1);
    engine.plan_hir_statement(hir).unwrap();
}

#[test]
fn combines_lowered_table_subqueries() {
    let schema = schema();
    let user_table = schema.mapping.model(User::id()).table;
    let post_table = schema.mapping.model(Post::id()).table;
    let column = |index| {
        Expr::Reference(stmt::ExprReference::Column(stmt::ExprColumn {
            nesting: 0,
            table: 0,
            column: index,
        }))
    };
    let name = Expr::eq(column(1), "alice");
    let active = column(2);
    let membership = |filter| {
        let mut query = Query::new_select(user_table, filter);
        query.body.as_select_mut_unwrap().returning = Returning::Project(column(0));
        Expr::in_subquery(column(1), query)
    };
    let mut outer = Query::new_select(
        post_table,
        Expr::and(membership(name.clone()), membership(active.clone())),
    );
    Simplify::new(&schema, &Capability::SQLITE).visit_stmt_query_mut(&mut outer);
    assert_eq!(
        outer.body.as_select_unwrap().filter.as_expr(),
        &membership(Expr::and(name, active))
    );
}

#[test]
fn nullable_filter_context_propagates_through_or_but_not_returning() {
    let nullable = |filter| Expr::in_subquery(field::<Post>(3), users(filter));
    let input = Expr::and(nullable(name()), nullable(active()));
    let unrelated = Expr::eq(field::<Post>(0), 42);
    assert_eq!(
        simplify_filter(Expr::or(input.clone(), unrelated.clone())),
        Expr::or(nullable(Expr::and(name(), active())), unrelated)
    );

    let mut outer = query(Post::id(), input.clone(), input.clone());
    Simplify::new(&schema(), &Capability::SQLITE).visit_stmt_query_mut(&mut outer);
    assert_eq!(
        outer.body.as_select_unwrap().returning,
        Returning::Project(input)
    );
}

#[test]
fn preserves_unstable_and_locked_nested_queries() {
    let mut locked = users(name());
    locked.locks.push(stmt::Lock::Update);
    let unstable = users(Expr::eq(field::<User>(0), Expr::last_insert_id()));
    for nested in [locked, unstable] {
        let input = Expr::and(
            membership(name()),
            membership(Expr::in_subquery(field::<User>(0), nested)),
        );
        assert_eq!(simplify_filter(input.clone()), input);
    }
}

#[test]
fn preserves_ordering_and_ctes() {
    let mut ordered = users(active());
    ordered.order_by = Some(stmt::OrderBy {
        exprs: vec![stmt::OrderByExpr {
            expr: field::<User>(0),
            order: Some(stmt::Direction::Asc),
        }],
    });
    let mut with = users(active());
    with.with = Some(stmt::With {
        ctes: vec![stmt::Cte {
            query: users(name()),
        }],
    });
    for query in [ordered, with] {
        let input = Expr::and(
            membership(name()),
            Expr::in_subquery(field::<Post>(1), query),
        );
        assert_eq!(simplify_filter(input.clone()), input);
    }
}
