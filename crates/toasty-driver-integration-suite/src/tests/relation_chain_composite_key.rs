//! Chain relation methods on `Many` where one or more hops in the chain has
//! a composite key. Parallels [`crate::tests::relation_chain`] which covers
//! the all-single-key case.
//!
//! Two positions are interesting:
//!
//! - A `BelongsTo` second hop whose foreign key spans multiple columns.
//! - A `HasMany` first hop whose paired `BelongsTo` on the target has a
//!   composite foreign key.
//!
//! The shared scenario [`crate::scenarios::composite_chain_relations`]
//! arranges `User → Todo → Category` so both positions are reachable from a
//! single dataset: `Category` has composite PK `(id, revision)` and Todo's
//! FK to it spans `(category_id, category_revision)`. The Todo→User FK is
//! single-column, so `category.todos().user()` cross-checks that a composite
//! first hop chains cleanly into a single-column second hop.

use crate::prelude::*;
use crate::scenarios::composite_chain_relations::Category;

/// Insert a Category with both fields of its composite PK set. The scenario
/// uses a non-auto composite PK so callers have to pick `id` and `revision`
/// themselves; centralising that here keeps the tests focused on chain
/// behavior rather than key plumbing.
async fn make_category(db: &mut toasty::Db, name: &str, revision: i64) -> Result<Category> {
    toasty::create!(Category {
        id: uuid::Uuid::new_v4(),
        revision,
        name,
    })
    .exec(db)
    .await
}

// =====================================================================
// `user.todos().category()` — second hop is BelongsTo with composite FK
// =====================================================================

/// Happy path mirroring `relation_chain::user_todos_category` but the
/// `Todo → Category` belongs_to spans `(category_id, category_revision)`.
/// The result must dedupe on the composite key, not just on `id`.
#[driver_test(scenario(crate::scenarios::composite_chain_relations))]
pub async fn user_todos_category_composite(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User { name: "Anchovy" })
        .exec(&mut db)
        .await?;
    let other_user = toasty::create!(User { name: "Other" })
        .exec(&mut db)
        .await?;

    let food = make_category(&mut db, "Food", 1).await?;
    let drink = make_category(&mut db, "Drink", 1).await?;
    let _unused = make_category(&mut db, "Unused", 1).await?;

    toasty::create!(Todo::[
        { title: "salad", user: &user, category: &food },
        { title: "tea",   user: &user, category: &drink },
        { title: "sushi", user: &user, category: &food },
        { title: "wine",  user: &other_user, category: &drink },
    ])
    .exec(&mut db)
    .await?;

    let mut categories = user.todos().category().exec(&mut db).await?;
    categories.sort_by_key(|c| c.name.clone());

    let ids: Vec<_> = categories.iter().map(|c| (c.id, c.revision)).collect();
    assert_unique!(ids);
    assert_eq!(categories.len(), 2);
    assert_eq!(
        (categories[0].id, categories[0].revision),
        (drink.id, drink.revision)
    );
    assert_eq!(
        (categories[1].id, categories[1].revision),
        (food.id, food.revision)
    );
    Ok(())
}

/// Two Categories that share `id` but differ in `revision` must both come
/// back as distinct rows from a chain query. This protects against a
/// regression where the IN-subquery was generated over `category_id` only,
/// silently merging different revisions.
#[driver_test(scenario(crate::scenarios::composite_chain_relations))]
pub async fn user_todos_category_distinguishes_by_revision(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User { name: "Owner" })
        .exec(&mut db)
        .await?;

    // Same `id`, different `revision` — without composite-key handling
    // these would collide in the FK subquery.
    let shared_id = uuid::Uuid::new_v4();
    let v1 = toasty::create!(Category {
        id: shared_id,
        revision: 1,
        name: "v1",
    })
    .exec(&mut db)
    .await?;
    let v2 = toasty::create!(Category {
        id: shared_id,
        revision: 2,
        name: "v2",
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Todo::[
        { title: "old", user: &user, category: &v1 },
        { title: "new", user: &user, category: &v2 },
    ])
    .exec(&mut db)
    .await?;

    let mut categories = user.todos().category().exec(&mut db).await?;
    categories.sort_by_key(|c| c.revision);

    assert_eq!(categories.len(), 2);
    assert_eq!(
        (categories[0].id, categories[0].revision),
        (v1.id, v1.revision)
    );
    assert_eq!(
        (categories[1].id, categories[1].revision),
        (v2.id, v2.revision)
    );
    Ok(())
}

/// A chain whose source produces no rows (`user` has no todos) must
/// short-circuit to an empty result, even when there is unrelated data of
/// the matching shape in the table.
#[driver_test(scenario(crate::scenarios::composite_chain_relations))]
pub async fn composite_chain_from_empty_source_is_empty(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let lonely = toasty::create!(User { name: "Lonely" })
        .exec(&mut db)
        .await?;

    let busy = toasty::create!(User { name: "Busy" }).exec(&mut db).await?;
    let cat = make_category(&mut db, "Solo", 1).await?;
    toasty::create!(Todo {
        title: "salad",
        user: &busy,
        category: &cat,
    })
    .exec(&mut db)
    .await?;

    let categories = lonely.todos().category().exec(&mut db).await?;
    assert!(categories.is_empty());
    Ok(())
}

/// Filter applied to the chain's terminal model narrows the chain's result.
/// Mirrors `relation_chain::chain_then_filter` but the terminal model has a
/// composite PK.
#[driver_test(scenario(crate::scenarios::composite_chain_relations))]
pub async fn composite_chain_then_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User { name: "Filty" })
        .exec(&mut db)
        .await?;
    let food = make_category(&mut db, "Food", 1).await?;
    let drink = make_category(&mut db, "Drink", 1).await?;

    toasty::create!(Todo::[
        { title: "salad", user: &user, category: &food },
        { title: "tea",   user: &user, category: &drink },
    ])
    .exec(&mut db)
    .await?;

    let only_food = user
        .todos()
        .category()
        .filter(Category::fields().name().eq("Food"))
        .exec(&mut db)
        .await?;
    assert_eq!(only_food.len(), 1);
    assert_eq!(
        (only_food[0].id, only_food[0].revision),
        (food.id, food.revision)
    );
    Ok(())
}

// =====================================================================
// `category.todos()` — first hop is HasMany whose pair is composite-FK
// =====================================================================

/// Chain starting at a model with a composite PK. The first hop's pair
/// (Todo's `belongs_to` back to Category) is composite, so the filter
/// generated for `Todo.category_id` must include both `category_id` and
/// `category_revision`.
#[driver_test(scenario(crate::scenarios::composite_chain_relations))]
pub async fn category_todos_user_composite_first_hop(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bob = toasty::create!(User { name: "Bob" }).exec(&mut db).await?;

    let food = make_category(&mut db, "Food", 1).await?;
    let other = make_category(&mut db, "Other", 1).await?;

    toasty::create!(Todo::[
        { title: "salad", user: &alice, category: &food },
        { title: "tea",   user: &bob,   category: &food },
        { title: "wine",  user: &alice, category: &other },
    ])
    .exec(&mut db)
    .await?;

    let mut users = food.todos().user().exec(&mut db).await?;
    users.sort_by_key(|u| u.name.clone());

    let ids: Vec<_> = users.iter().map(|u| u.id).collect();
    assert_unique!(ids);
    assert_eq!(users.len(), 2);
    assert_eq!(users[0].name, "Alice");
    assert_eq!(users[1].name, "Bob");
    Ok(())
}

/// `Category(id=X, revision=1)` and `Category(id=X, revision=2)` are two
/// distinct categories. The chain starting at one must not pick up the
/// other's todos — the filter has to discriminate on both FK columns.
#[driver_test(scenario(crate::scenarios::composite_chain_relations))]
pub async fn category_todos_respects_revision(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bob = toasty::create!(User { name: "Bob" }).exec(&mut db).await?;

    // Two categories sharing the same `id`, differing only in `revision`.
    let shared_id = uuid::Uuid::new_v4();
    let v1 = toasty::create!(Category {
        id: shared_id,
        revision: 1,
        name: "v1",
    })
    .exec(&mut db)
    .await?;
    let v2 = toasty::create!(Category {
        id: shared_id,
        revision: 2,
        name: "v2",
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Todo::[
        { title: "for-v1", user: &alice, category: &v1 },
        { title: "for-v2", user: &bob,   category: &v2 },
    ])
    .exec(&mut db)
    .await?;

    let v1_users = v1.todos().user().exec(&mut db).await?;
    assert_eq!(v1_users.len(), 1);
    assert_eq!(v1_users[0].name, "Alice");

    let v2_users = v2.todos().user().exec(&mut db).await?;
    assert_eq!(v2_users.len(), 1);
    assert_eq!(v2_users[0].name, "Bob");
    Ok(())
}

/// A `category.todos()` chain that ends in `.filter(...)` narrows the
/// terminal model the same way as in the single-key chain tests. This
/// pairs with `composite_chain_then_filter` (which exercises the
/// second-hop composite case) to make sure both endpoints respect filters.
#[driver_test(scenario(crate::scenarios::composite_chain_relations))]
pub async fn category_todos_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bob = toasty::create!(User { name: "Bob" }).exec(&mut db).await?;

    let food = make_category(&mut db, "Food", 1).await?;

    toasty::create!(Todo::[
        { title: "salad",   user: &alice, category: &food },
        { title: "sushi",   user: &bob,   category: &food },
    ])
    .exec(&mut db)
    .await?;

    let just_salad = food
        .todos()
        .filter(Todo::fields().title().eq("salad"))
        .exec(&mut db)
        .await?;
    assert_eq!(just_salad.len(), 1);
    assert_eq!(just_salad[0].title, "salad");
    Ok(())
}

/// `Category::filter(name = ...).todos().user()` — the chain's source query
/// is filtered by a *non-FK* column (`name`). The fallback path inside
/// `lift_belongs_to_in_subquery` (which can't lift the filter onto FK
/// columns) must still produce a working composite-FK IN subquery rather
/// than panicking on `todo!("composite keys")`.
#[driver_test(scenario(crate::scenarios::composite_chain_relations))]
pub async fn filtered_category_todos_user_composite(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bob = toasty::create!(User { name: "Bob" }).exec(&mut db).await?;

    let food = make_category(&mut db, "Food", 1).await?;
    let drink = make_category(&mut db, "Drink", 1).await?;

    toasty::create!(Todo::[
        { title: "salad", user: &alice, category: &food },
        { title: "tea",   user: &bob,   category: &drink },
    ])
    .exec(&mut db)
    .await?;

    let mut users = Category::filter(Category::fields().name().eq("Food"))
        .todos()
        .user()
        .exec(&mut db)
        .await?;
    users.sort_by_key(|u| u.name.clone());

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");
    Ok(())
}

// =====================================================================
// Both hops composite — HasMany→HasMany where parent has composite PK
// =====================================================================

/// `Author → posts → comments` with composite keys on `Author` and `Post`.
/// The chain involves two HasMany hops, each producing an IN-subquery
/// against a composite-FK pair. Mirrors `relation_chain::has_many_through_has_many`.
#[driver_test]
pub async fn composite_has_many_through_has_many(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id, revision)]
    struct Author {
        id: uuid::Uuid,
        revision: i64,
        name: String,
        #[has_many]
        posts: toasty::HasMany<Post>,
    }

    #[derive(Debug, toasty::Model)]
    #[index(author_id, author_revision)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,
        author_id: uuid::Uuid,
        author_revision: i64,
        #[belongs_to(key = [author_id, author_revision], references = [id, revision])]
        author: toasty::BelongsTo<Author>,
        title: String,
        #[has_many]
        comments: toasty::HasMany<Comment>,
    }

    #[derive(Debug, toasty::Model)]
    struct Comment {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        post_id: uuid::Uuid,
        #[belongs_to(key = post_id, references = id)]
        post: toasty::BelongsTo<Post>,
        body: String,
    }

    let mut db = test.setup_db(models!(Author, Post, Comment)).await;

    let alice = toasty::create!(Author {
        id: uuid::Uuid::new_v4(),
        revision: 1,
        name: "Alice",
    })
    .exec(&mut db)
    .await?;
    let bob = toasty::create!(Author {
        id: uuid::Uuid::new_v4(),
        revision: 1,
        name: "Bob",
    })
    .exec(&mut db)
    .await?;

    let p1 = toasty::create!(Post {
        title: "p1",
        author: &alice
    })
    .exec(&mut db)
    .await?;
    let p2 = toasty::create!(Post {
        title: "p2",
        author: &alice
    })
    .exec(&mut db)
    .await?;
    let _p3 = toasty::create!(Post {
        title: "p3",
        author: &bob
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Comment::[
        { body: "c1", post: &p1 },
        { body: "c2", post: &p1 },
        { body: "c3", post: &p2 },
    ])
    .exec(&mut db)
    .await?;

    let mut alice_comments = alice.posts().comments().exec(&mut db).await?;
    alice_comments.sort_by_key(|c| c.body.clone());
    let bodies: Vec<_> = alice_comments.into_iter().map(|c| c.body).collect();
    assert_eq!(bodies, vec!["c1", "c2", "c3"]);

    let bob_comments = bob.posts().comments().exec(&mut db).await?;
    assert!(bob_comments.is_empty());
    Ok(())
}
