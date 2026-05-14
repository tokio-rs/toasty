use toasty::stmt::{IntoScope, IntoStatement, List, Query};
use toasty_core::stmt as core_stmt;

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: i64,
    name: String,
}

fn untyped_query<T>(stmt: toasty::Statement<T>) -> core_stmt::Query {
    stmt.into_untyped().into_query_unwrap()
}

#[test]
fn into_scope_from_list_preserves_query() {
    let q: Query<List<User>> = Query::all();
    let original = untyped_query(q.clone().into_statement());

    let widened = untyped_query(IntoScope::<User>::into_scope(q));

    assert_eq!(widened, original);
    assert!(!widened.single);
    assert!(widened.limit.is_none());
}

#[test]
fn into_scope_from_option_clears_single_and_limit() {
    let q: Query<Option<User>> = Query::<List<User>>::all().first();

    // Sanity: .first() applied both `single = true` and `LIMIT 1`.
    let pre = untyped_query(q.clone().into_statement());
    assert!(pre.single);
    assert!(pre.limit.is_some());

    let widened = untyped_query(IntoScope::<User>::into_scope(q));

    assert!(!widened.single);
    assert!(widened.limit.is_none());
}

#[test]
fn into_scope_from_one_clears_single_and_limit() {
    let q: Query<User> = Query::<List<User>>::all().one();

    let pre = untyped_query(q.clone().into_statement());
    assert!(pre.single);
    assert!(pre.limit.is_some());

    let widened = untyped_query(IntoScope::<User>::into_scope(q));

    assert!(!widened.single);
    assert!(widened.limit.is_none());
}

#[test]
fn into_scope_preserves_filter_body() {
    // The body (source + filter) should round-trip identically through
    // widening — only `single` / `limit` get reset.
    let list_q: Query<List<User>> = Query::filter(User::fields().name().eq("alice"));
    let from_list = untyped_query(IntoScope::<User>::into_scope(list_q.clone()));

    let from_option = untyped_query(IntoScope::<User>::into_scope(list_q.clone().first()));
    let from_one = untyped_query(IntoScope::<User>::into_scope(list_q.one()));

    assert_eq!(from_list.body, from_option.body);
    assert_eq!(from_list.body, from_one.body);
}
