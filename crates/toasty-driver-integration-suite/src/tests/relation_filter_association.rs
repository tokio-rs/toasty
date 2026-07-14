//! Tests for filtering models by conditions on associated (HasOne, BelongsTo) fields.

use crate::prelude::*;

/// Filter a parent by a HasOne field, with eq and a non-commutative op.
#[driver_test(id(ID), requires(sql))]
pub async fn filter_by_has_one_field(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        profile: toasty::Deferred<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        score: i64,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<Option<User>>,
    }

    let mut db = t.setup_db(models!(User, Profile)).await;

    toasty::create!(User::[
        { name: "alice", profile: { score: 80 } },
        { name: "bob",   profile: { score: 30 } },
    ])
    .exec(&mut db)
    .await?;

    let users: Vec<User> = User::filter(User::fields().profile().score().eq(80))
        .exec(&mut db)
        .await?;
    assert_eq!(
        users.iter().map(|u| u.name.as_str()).collect::<Vec<_>>(),
        ["alice"]
    );

    let users: Vec<User> = User::filter(User::fields().profile().score().gt(50))
        .exec(&mut db)
        .await?;
    assert_eq!(
        users.iter().map(|u| u.name.as_str()).collect::<Vec<_>>(),
        ["alice"]
    );

    Ok(())
}

/// Filter a child by a BelongsTo field.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::has_one_optional_belongs_to)
)]
pub async fn filter_by_belongs_to_field(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User::[
        { name: "alice", profile: { bio: "alice's bio" } },
        { name: "bob",   profile: { bio: "bob's bio" } },
    ])
    .exec(&mut db)
    .await?;

    let profiles: Vec<Profile> = Profile::filter(Profile::fields().user().name().eq("alice"))
        .exec(&mut db)
        .await?;
    assert_eq!(
        profiles.iter().map(|p| p.bio.as_str()).collect::<Vec<_>>(),
        ["alice's bio"]
    );

    Ok(())
}

/// Filters through a relation field with `.like()`, e.g. finding profiles
/// whose user's name starts with "al". `LIKE` lowers to `Expr::Like`, not
/// `Expr::BinaryOp`, so the rewrite that turns a relation-path comparison into
/// a foreign-key subquery has to handle it explicitly; without that the query
/// panics.
///
/// Covers both relation directions: `BelongsTo` (child filtered by parent)
/// and `Has` (parent filtered by child).
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::has_one_optional_belongs_to)
)]
pub async fn filter_by_relation_field_like(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User::[
        { name: "alice", profile: { bio: "alice's bio" } },
        { name: "bob",   profile: { bio: "bob's bio" } },
    ])
    .exec(&mut db)
    .await?;

    // BelongsTo: filter children by a LIKE on the parent's field.
    let profiles: Vec<Profile> = Profile::filter(Profile::fields().user().name().like("al%"))
        .exec(&mut db)
        .await?;
    assert_eq!(
        profiles.iter().map(|p| p.bio.as_str()).collect::<Vec<_>>(),
        ["alice's bio"]
    );

    // Has: filter parents by a LIKE on the child's field.
    let users: Vec<User> = User::filter(User::fields().profile().bio().like("%'s bio"))
        .exec(&mut db)
        .await?;
    let mut names: Vec<_> = users.iter().map(|u| u.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["alice", "bob"]);

    Ok(())
}

/// Filter through three chained HasOne associations: `A.b.c.name == ...`.
#[driver_test(id(ID), requires(sql))]
pub async fn filter_by_nested_has_one_chain(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct A {
        #[key]
        #[auto]
        id: ID,

        label: String,

        #[has_one]
        b: toasty::Deferred<Option<B>>,
    }

    #[derive(Debug, toasty::Model)]
    struct B {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        a_id: Option<ID>,

        #[belongs_to(key = a_id, references = id)]
        a: toasty::Deferred<Option<A>>,

        #[has_one]
        c: toasty::Deferred<Option<C>>,
    }

    #[derive(Debug, toasty::Model)]
    struct C {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[unique]
        b_id: Option<ID>,

        #[belongs_to(key = b_id, references = id)]
        b: toasty::Deferred<Option<B>>,
    }

    let mut db = t.setup_db(models!(A, B, C)).await;

    toasty::create!(A::[
        { label: "match",    b: { c: { name: "target" } } },
        { label: "no-match", b: { c: { name: "other" } } },
    ])
    .exec(&mut db)
    .await?;

    let results: Vec<A> = A::filter(A::fields().b().c().name().eq("target"))
        .exec(&mut db)
        .await?;
    assert_eq!(
        results.iter().map(|a| a.label.as_str()).collect::<Vec<_>>(),
        ["match"]
    );

    Ok(())
}

/// Filter through `BelongsTo → HasMany` with `.any()` — the queried model's
/// parent owns a collection, and the filter is a quantifier over that
/// collection (here, "sibling" todos sharing the same category).
///
/// Path `todo.category.todos.any(title == "salad")` lowers to an
/// `Expr::InSubquery` whose LHS is an `Expr::Project` through the
/// `BelongsTo`. Single-hop `parent.children.any(...)` already lifts (see
/// `relation_has_many_filter::filter_parent_by_child_field`); previously the
/// `BelongsTo`-then-`HasMany` chain hit a `todo!()` in
/// `LiftInSubquery::lift_in_subquery`.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::has_many_multi_relation)
)]
pub async fn filter_by_belongs_to_has_many_any(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let food = toasty::create!(Category { name: "food" })
        .exec(&mut db)
        .await?;
    let drink = toasty::create!(Category { name: "drink" })
        .exec(&mut db)
        .await?;
    let user = toasty::create!(User { name: "Anchovy" })
        .exec(&mut db)
        .await?;

    toasty::create!(Todo::[
        { title: "salad", user: &user, category: &food  },
        { title: "sushi", user: &user, category: &food  },
        { title: "tea",   user: &user, category: &drink },
    ])
    .exec(&mut db)
    .await?;

    // Todos whose category has at least one todo titled "salad" — i.e.,
    // every todo in the "food" category.
    let todos: Vec<Todo> = Todo::filter(
        Todo::fields()
            .category()
            .todos()
            .any(Todo::fields().title().eq("salad")),
    )
    .exec(&mut db)
    .await?;

    assert_eq_unordered!(todos.iter().map(|t| &t.title[..]), ["salad", "sushi"]);

    Ok(())
}
