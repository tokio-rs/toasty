use std::sync::Arc;

use toasty_core::{
    driver::Capability,
    schema::app::VariantId,
    stmt::{self, Expr, Query, Statement},
};

use crate as toasty;
use crate::{
    engine::{Engine, test_util::test_schema_with},
    schema::{Embed, Model},
};

#[derive(Debug, toasty::Embed)]
enum Inner {
    A { value: i64 },
    B,
}

#[derive(Debug, toasty::Embed)]
enum Owner {
    Primary {
        #[shared(id)]
        id: i64,
        note: Option<String>,
        inner: Inner,
    },
    Other {
        #[shared(id)]
        id: i64,
    },
}

#[derive(Debug, toasty::Model)]
struct Object {
    #[key]
    id: i64,
    lhs: Owner,
    rhs: Owner,
}

const PRIMARY: usize = 0;

fn engine() -> Engine {
    let schema = Arc::new(test_schema_with(&[
        Object::schema(),
        Owner::schema(),
        Inner::schema(),
    ]));
    Engine::new(schema, &Capability::SQLITE)
}

fn normalize_stmt(stmt: &mut Statement) {
    engine().normalize_stmt(stmt).unwrap();
}

/// Normalizes a query on `Object` filtered by `filter` and returns the
/// normalized filter.
fn normalize(filter: impl Into<Expr>) -> Expr {
    let mut stmt = Statement::Query(Query::new_select(Object::id(), filter.into()));
    normalize_stmt(&mut stmt);
    filter_of(&stmt)
}

fn filter_of(stmt: &Statement) -> Expr {
    let select = stmt.as_query().unwrap().body.as_select_unwrap();
    select.filter.expr.clone().unwrap()
}

fn path_expr<U>(path: impl Into<crate::stmt::Path<Object, U>>) -> Expr {
    stmt::Path::from(path.into()).into_stmt()
}

fn owner(index: usize) -> VariantId {
    VariantId {
        model: Owner::id(),
        index,
    }
}

fn lhs_is_primary() -> Expr {
    Expr::is_variant(path_expr(Object::fields().lhs()), owner(PRIMARY))
}

fn rhs_is_primary() -> Expr {
    Expr::is_variant(path_expr(Object::fields().rhs()), owner(PRIMARY))
}

fn lhs_id() -> Expr {
    path_expr(Object::fields().lhs().primary().id())
}

fn lhs_note() -> Expr {
    path_expr(Object::fields().lhs().primary().note())
}

#[test]
fn comparison_requires_the_selected_variant() {
    let filter = normalize(Object::fields().lhs().primary().id().eq(1));

    assert_eq!(filter, Expr::and(lhs_is_primary(), Expr::eq(lhs_id(), 1)));
}

#[test]
fn guards_are_collected_from_both_operands() {
    let rhs_id = || path_expr(Object::fields().rhs().primary().id());
    let filter = normalize(
        Object::fields()
            .lhs()
            .primary()
            .id()
            .ne(Object::fields().rhs().primary().id()),
    );

    assert_eq!(
        filter,
        Expr::and_from_vec(vec![
            lhs_is_primary(),
            rhs_is_primary(),
            Expr::ne(lhs_id(), rhs_id()),
        ])
    );
}

#[test]
fn identical_selections_guard_once() {
    let filter = normalize(Expr::eq(lhs_id(), lhs_id()));

    assert_eq!(
        filter,
        Expr::and(lhs_is_primary(), Expr::eq(lhs_id(), lhs_id()))
    );
}

#[test]
fn nested_selections_guard_every_enclosing_variant() {
    let inner_is_a = Expr::is_variant(
        path_expr(Object::fields().lhs().primary().inner()),
        VariantId {
            model: Inner::id(),
            index: 0,
        },
    );
    let value = path_expr(Object::fields().lhs().primary().inner().a().value());
    let filter = normalize(Expr::eq(value.clone(), 1));

    // Outermost variant first: the inner check is only meaningful once the
    // outer variant holds.
    assert_eq!(
        filter,
        Expr::and_from_vec(vec![lhs_is_primary(), inner_is_a, Expr::eq(value, 1)])
    );
}

#[test]
fn negating_a_predicate_negates_the_guarded_predicate() {
    let filter = normalize(Object::fields().lhs().primary().id().eq(1).not());

    assert_eq!(
        filter,
        Expr::not(Expr::and(lhs_is_primary(), Expr::eq(lhs_id(), 1)))
    );
}

#[test]
fn negative_null_check_is_guarded_as_a_whole() {
    // `is_some()` is a predicate: the guard applies to it.
    let filter = normalize(Object::fields().lhs().primary().note().is_some());
    assert_eq!(
        filter,
        Expr::and(lhs_is_primary(), Expr::not(Expr::is_null(lhs_note())))
    );

    // `is_none().not()` negates the guarded predicate instead.
    let filter = normalize(Object::fields().lhs().primary().note().is_none().not());
    assert_eq!(
        filter,
        Expr::not(Expr::and(lhs_is_primary(), Expr::is_null(lhs_note())))
    );
}

#[test]
fn negative_membership_is_guarded_as_a_whole() {
    let subquery = || {
        Query::new_select(
            Object::id(),
            Expr::eq(Expr::ref_self_field(Object::id().field(0)), 1),
        )
    };
    let filter = normalize(Expr::not_in_subquery(lhs_id(), subquery()));

    assert_eq!(
        filter,
        Expr::and(
            lhs_is_primary(),
            Expr::not(Expr::in_subquery(lhs_id(), subquery()))
        )
    );
}

#[test]
fn nested_predicate_is_guarded_at_its_own_boundary() {
    let filter = normalize(Object::fields().lhs().primary().note().is_none().eq(false));

    assert_eq!(
        filter,
        Expr::eq(
            Expr::and(lhs_is_primary(), Expr::is_null(lhs_note())),
            false
        )
    );
}

#[test]
fn subquery_predicates_keep_their_own_guards() {
    let outer = Expr::ref_self_field(Object::id().field(0));
    let guarded_subquery = || {
        Query::new_select(
            Object::id(),
            Expr::and(lhs_is_primary(), Expr::eq(lhs_id(), 1)),
        )
    };

    // The selection inside the subquery is guarded there, not outside.
    let membership = Expr::in_subquery(
        outer.clone(),
        Query::new_select(Object::id(), Expr::eq(lhs_id(), 1)),
    );
    assert_eq!(
        normalize(membership),
        Expr::in_subquery(outer.clone(), guarded_subquery())
    );

    // A guard in force outside the subquery does not reach into it.
    let membership = Expr::in_subquery(
        outer.clone(),
        Query::new_select(Object::id(), Expr::eq(lhs_id(), 1)),
    );
    assert_eq!(
        normalize(Expr::and(lhs_is_primary(), membership)),
        Expr::and(
            lhs_is_primary(),
            Expr::in_subquery(outer, guarded_subquery())
        )
    );
}

#[test]
fn guards_flatten_into_the_enclosing_conjunction() {
    let unrelated = || Expr::eq(Expr::ref_self_field(Object::id().field(0)), 1);
    let rhs_id = || path_expr(Object::fields().rhs().primary().id());
    let filter = normalize(Expr::and_from_vec(vec![
        unrelated(),
        Expr::eq(lhs_id(), 1),
        Expr::eq(rhs_id(), 2),
    ]));

    // Each guard is a sibling of the comparison it scopes.
    assert_eq!(
        filter,
        Expr::and_from_vec(vec![
            unrelated(),
            lhs_is_primary(),
            Expr::eq(lhs_id(), 1),
            rhs_is_primary(),
            Expr::eq(rhs_id(), 2),
        ])
    );
}

#[test]
fn conjunction_does_not_repeat_a_guard_it_states() {
    // `matches()` combines the variant check with its body explicitly, so
    // normalization has nothing to add.
    let filter = Object::fields()
        .lhs()
        .primary()
        .matches(|primary| primary.id().eq(1).and(primary.id().ne(2)));
    let explicit = Expr::from(filter.clone());
    let Expr::And(operands) = &explicit else {
        panic!("expected a conjunction, got {explicit:?}");
    };
    assert_eq!(operands.len(), 3);

    assert_eq!(normalize(filter), explicit);
}

#[test]
fn ordering_selections_stay_unguarded() {
    let mut query = Query::new_select(Object::id(), true);
    query.order_by = Some(Object::fields().lhs().primary().id().asc().into());
    let mut stmt = Statement::Query(query);
    let before = stmt.clone();

    normalize_stmt(&mut stmt);

    assert_eq!(stmt, before);
}

#[test]
fn guard_normalization_is_idempotent() {
    let filter = Object::fields()
        .lhs()
        .primary()
        .id()
        .ne(Object::fields().rhs().primary().id())
        .and(Object::fields().lhs().primary().note().is_some())
        .and(Object::fields().lhs().primary().id().eq(1).not())
        .or(Object::fields().lhs().primary().inner().a().value().eq(2));
    let mut stmt = Statement::Query(Query::new_select(Object::id(), filter));

    normalize_stmt(&mut stmt);
    let once = stmt.clone();
    normalize_stmt(&mut stmt);

    assert_eq!(stmt, once);
}
