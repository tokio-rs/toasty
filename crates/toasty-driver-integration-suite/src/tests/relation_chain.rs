//! Chain relation methods on a `Many` handle to traverse multi-step
//! associations without declaring a `via` relation on the schema.
//!
//! `user.todos().category()` produces an `Association` whose path is two
//! steps long (`User → todos → category`). The query engine lowers this by
//! unfolding into nested IN-subqueries against the outermost relation.

use crate::prelude::*;

/// Happy path: HasMany → BelongsTo chain returns the distinct set of
/// categories the user's todos belong to, with no duplicates even when the
/// user has multiple todos in the same category.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_multi_relation))]
pub async fn user_todos_category(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User { name: "Anchovy" })
        .exec(&mut db)
        .await?;
    let other_user = toasty::create!(User { name: "Other" })
        .exec(&mut db)
        .await?;

    let food = toasty::create!(Category { name: "Food" })
        .exec(&mut db)
        .await?;
    let drink = toasty::create!(Category { name: "Drink" })
        .exec(&mut db)
        .await?;
    let unused = toasty::create!(Category { name: "Unused" })
        .exec(&mut db)
        .await?;

    toasty::create!(Todo::[
        { title: "salad", user: &user, category: &food },
        { title: "tea",   user: &user, category: &drink },
        { title: "sushi", user: &user, category: &food },
        { title: "wine",  user: &other_user, category: &unused },
    ])
    .exec(&mut db)
    .await?;

    let mut categories = user.todos().category().exec(&mut db).await?;
    categories.sort_by_key(|c| c.name.clone());

    let ids: Vec<_> = categories.iter().map(|c| c.id).collect();
    assert_unique!(ids);
    assert_eq!(categories.len(), 2);
    assert_eq!(categories[0].id, drink.id);
    assert_eq!(categories[1].id, food.id);
    Ok(())
}

/// Empty source: a user with no todos produces an empty chain result.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_multi_relation))]
pub async fn chain_from_empty_source_is_empty(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User { name: "Lonely" })
        .exec(&mut db)
        .await?;

    // Another user with todos in some category, to ensure the data is non-empty
    // overall but isolated from `user`.
    let other = toasty::create!(User { name: "Busy" }).exec(&mut db).await?;
    let food = toasty::create!(Category { name: "Food" })
        .exec(&mut db)
        .await?;
    toasty::create!(Todo {
        title: "salad",
        user: &other,
        category: &food
    })
    .exec(&mut db)
    .await?;

    let categories = user.todos().category().exec(&mut db).await?;
    assert!(categories.is_empty());
    Ok(())
}

/// Many todos sharing a single category yield exactly one category row in the
/// chain result — IN dedupes against the outermost relation.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_multi_relation))]
pub async fn chain_dedupes_when_todos_share_category(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User { name: "Cooky" })
        .exec(&mut db)
        .await?;
    let food = toasty::create!(Category { name: "Food" })
        .exec(&mut db)
        .await?;

    for i in 0..5 {
        let title = format!("todo {i}");
        toasty::create!(Todo {
            title,
            user: &user,
            category: &food
        })
        .exec(&mut db)
        .await?;
    }

    let categories = user.todos().category().exec(&mut db).await?;
    assert_eq!(categories.len(), 1);
    assert_eq!(categories[0].id, food.id);
    Ok(())
}

/// The chain respects the starting source: each user's chain returns only the
/// categories their own todos belong to, even when the data sets overlap.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_multi_relation))]
pub async fn chain_scopes_per_starting_user(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bob = toasty::create!(User { name: "Bob" }).exec(&mut db).await?;

    let a = toasty::create!(Category { name: "A" })
        .exec(&mut db)
        .await?;
    let b = toasty::create!(Category { name: "B" })
        .exec(&mut db)
        .await?;
    let c = toasty::create!(Category { name: "C" })
        .exec(&mut db)
        .await?;

    toasty::create!(Todo::[
        { title: "a1", user: &alice, category: &a },
        { title: "a2", user: &alice, category: &b },
        { title: "b1", user: &bob, category: &b },
        { title: "b2", user: &bob, category: &c },
    ])
    .exec(&mut db)
    .await?;

    let mut alice_cats = alice.todos().category().exec(&mut db).await?;
    alice_cats.sort_by_key(|c| c.name.clone());
    let alice_ids: Vec<_> = alice_cats.iter().map(|c| c.id).collect();
    assert_eq!(alice_ids, vec![a.id, b.id]);

    let mut bob_cats = bob.todos().category().exec(&mut db).await?;
    bob_cats.sort_by_key(|c| c.name.clone());
    let bob_ids: Vec<_> = bob_cats.iter().map(|c| c.id).collect();
    assert_eq!(bob_ids, vec![b.id, c.id]);
    Ok(())
}

/// `Many::filter(expr)` after a chain applies a filter to the final
/// relation. The result is the chain's category set narrowed by the filter.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_multi_relation))]
pub async fn chain_then_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User { name: "Filty" })
        .exec(&mut db)
        .await?;
    let food = toasty::create!(Category { name: "Food" })
        .exec(&mut db)
        .await?;
    let drink = toasty::create!(Category { name: "Drink" })
        .exec(&mut db)
        .await?;

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
    assert_eq!(only_food[0].id, food.id);
    Ok(())
}

/// Two HasMany hops in succession (`Author → posts → comments`). The lowering
/// unfolds into nested IN-subqueries on each `BelongsTo` pair.
#[driver_test]
pub async fn has_many_through_has_many(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Author {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        #[has_many]
        posts: toasty::HasMany<Post>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        author_id: uuid::Uuid,
        #[belongs_to(key = author_id, references = id)]
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

    let alice = toasty::create!(Author { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bob = toasty::create!(Author { name: "Bob" })
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
    let p3 = toasty::create!(Post {
        title: "p3",
        author: &bob
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Comment::[
        { body: "c1", post: &p1 },
        { body: "c2", post: &p1 },
        { body: "c3", post: &p2 },
        { body: "c4", post: &p3 },
    ])
    .exec(&mut db)
    .await?;

    let mut alice_comments = alice.posts().comments().exec(&mut db).await?;
    alice_comments.sort_by_key(|c| c.body.clone());
    let bodies: Vec<_> = alice_comments.iter().map(|c| c.body.clone()).collect();
    assert_eq!(bodies, vec!["c1", "c2", "c3"]);

    let bob_comments = bob.posts().comments().exec(&mut db).await?;
    assert_eq!(bob_comments.len(), 1);
    assert_eq!(bob_comments[0].body, "c4");
    Ok(())
}

/// A 3-step chain (`User → Project → Task → Tag`) walks the planner's
/// unfolder more than once. Verifies the recursive nesting and the chain of
/// `BelongsTo` rewrites at each hop.
#[driver_test]
pub async fn three_step_chain(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        #[has_many]
        projects: toasty::HasMany<Project>,
    }

    #[derive(Debug, toasty::Model)]
    struct Project {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        user_id: uuid::Uuid,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        name: String,
        #[has_many]
        tasks: toasty::HasMany<Task>,
    }

    #[derive(Debug, toasty::Model)]
    struct Task {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        project_id: uuid::Uuid,
        #[belongs_to(key = project_id, references = id)]
        project: toasty::BelongsTo<Project>,
        title: String,
        #[index]
        tag_id: uuid::Uuid,
        #[belongs_to(key = tag_id, references = id)]
        tag: toasty::BelongsTo<Tag>,
    }

    #[derive(Debug, toasty::Model)]
    struct Tag {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        #[has_many]
        tasks: toasty::HasMany<Task>,
    }

    let mut db = test.setup_db(models!(User, Project, Task, Tag)).await;

    let user = toasty::create!(User { name: "Owner" })
        .exec(&mut db)
        .await?;
    let other = toasty::create!(User { name: "Other" })
        .exec(&mut db)
        .await?;

    let backend = toasty::create!(Project {
        name: "Backend",
        user: &user
    })
    .exec(&mut db)
    .await?;
    let frontend = toasty::create!(Project {
        name: "Frontend",
        user: &user
    })
    .exec(&mut db)
    .await?;
    let unrelated = toasty::create!(Project {
        name: "Unrelated",
        user: &other
    })
    .exec(&mut db)
    .await?;

    let bug = toasty::create!(Tag { name: "bug" }).exec(&mut db).await?;
    let feat = toasty::create!(Tag { name: "feature" })
        .exec(&mut db)
        .await?;
    let chore = toasty::create!(Tag { name: "chore" }).exec(&mut db).await?;

    toasty::create!(Task::[
        { title: "fix login", project: &backend, tag: &bug },
        { title: "add dark mode", project: &frontend, tag: &feat },
        { title: "rotate keys", project: &backend, tag: &chore },
        { title: "different user", project: &unrelated, tag: &bug },
    ])
    .exec(&mut db)
    .await?;

    let mut tags = user.projects().tasks().tag().exec(&mut db).await?;
    tags.sort_by_key(|t| t.name.clone());
    let ids: Vec<_> = tags.iter().map(|t| t.id).collect();
    assert_unique!(ids);
    let names: Vec<_> = tags.iter().map(|t| t.name.clone()).collect();
    assert_eq!(names, vec!["bug", "chore", "feature"]);
    Ok(())
}

/// A 4-step chain (`Org → Team → Project → Issue → Tag`) drives the
/// `peel_first_step` loop through three iterations before reducing to a
/// single-step rewrite. Guards against regressions in the depth-independent
/// part of the unfolder.
#[driver_test]
pub async fn four_step_chain(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Org {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        #[has_many]
        teams: toasty::HasMany<Team>,
    }

    #[derive(Debug, toasty::Model)]
    struct Team {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        org_id: uuid::Uuid,
        #[belongs_to(key = org_id, references = id)]
        org: toasty::BelongsTo<Org>,
        name: String,
        #[has_many]
        projects: toasty::HasMany<Project>,
    }

    #[derive(Debug, toasty::Model)]
    struct Project {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        team_id: uuid::Uuid,
        #[belongs_to(key = team_id, references = id)]
        team: toasty::BelongsTo<Team>,
        name: String,
        #[has_many]
        issues: toasty::HasMany<Issue>,
    }

    #[derive(Debug, toasty::Model)]
    struct Issue {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        project_id: uuid::Uuid,
        #[belongs_to(key = project_id, references = id)]
        project: toasty::BelongsTo<Project>,
        title: String,
        #[index]
        tag_id: uuid::Uuid,
        #[belongs_to(key = tag_id, references = id)]
        tag: toasty::BelongsTo<Tag>,
    }

    #[derive(Debug, toasty::Model)]
    struct Tag {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        #[has_many]
        issues: toasty::HasMany<Issue>,
    }

    let mut db = test.setup_db(models!(Org, Team, Project, Issue, Tag)).await;

    let mine = toasty::create!(Org { name: "Mine" }).exec(&mut db).await?;
    let theirs = toasty::create!(Org { name: "Theirs" })
        .exec(&mut db)
        .await?;

    let core = toasty::create!(Team {
        name: "core",
        org: &mine
    })
    .exec(&mut db)
    .await?;
    let ops = toasty::create!(Team {
        name: "ops",
        org: &mine
    })
    .exec(&mut db)
    .await?;
    let outside = toasty::create!(Team {
        name: "outside",
        org: &theirs
    })
    .exec(&mut db)
    .await?;

    let backend = toasty::create!(Project {
        name: "backend",
        team: &core
    })
    .exec(&mut db)
    .await?;
    let frontend = toasty::create!(Project {
        name: "frontend",
        team: &core
    })
    .exec(&mut db)
    .await?;
    let infra = toasty::create!(Project {
        name: "infra",
        team: &ops
    })
    .exec(&mut db)
    .await?;
    let unrelated = toasty::create!(Project {
        name: "unrelated",
        team: &outside
    })
    .exec(&mut db)
    .await?;

    let bug = toasty::create!(Tag { name: "bug" }).exec(&mut db).await?;
    let feat = toasty::create!(Tag { name: "feature" })
        .exec(&mut db)
        .await?;
    let chore = toasty::create!(Tag { name: "chore" }).exec(&mut db).await?;
    let unused = toasty::create!(Tag { name: "unused" })
        .exec(&mut db)
        .await?;

    toasty::create!(Issue::[
        { title: "fix login", project: &backend, tag: &bug },
        { title: "dark mode", project: &frontend, tag: &feat },
        { title: "rotate keys", project: &infra, tag: &chore },
        { title: "duplicate", project: &backend, tag: &bug },
        { title: "their issue", project: &unrelated, tag: &unused },
    ])
    .exec(&mut db)
    .await?;

    let mut tags = mine.teams().projects().issues().tag().exec(&mut db).await?;
    tags.sort_by_key(|t| t.name.clone());

    let ids: Vec<_> = tags.iter().map(|t| t.id).collect();
    assert_unique!(ids);
    let names: Vec<_> = tags.iter().map(|t| t.name.clone()).collect();
    assert_eq!(names, vec!["bug", "chore", "feature"]);
    Ok(())
}

/// `BelongsTo<Option<_>>` in the chain skips `NULL` foreign keys. Todos with
/// no category contribute nothing to the chain.
#[driver_test]
pub async fn chain_skips_null_belongs_to(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        user_id: uuid::Uuid,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
        #[index]
        category_id: Option<uuid::Uuid>,
        #[belongs_to(key = category_id, references = id)]
        category: toasty::BelongsTo<Option<Category>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Category {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
    }

    let mut db = test.setup_db(models!(User, Todo, Category)).await;

    let user = toasty::create!(User { name: "Tester" })
        .exec(&mut db)
        .await?;
    let cat = toasty::create!(Category { name: "Only" })
        .exec(&mut db)
        .await?;

    toasty::create!(Todo::[
        { title: "with cat", user: &user, category: &cat },
        { title: "no cat 1", user: &user },
        { title: "no cat 2", user: &user },
    ])
    .exec(&mut db)
    .await?;

    let cats = user.todos().category().exec(&mut db).await?;
    assert_eq!(cats.len(), 1);
    assert_eq!(cats[0].id, cat.id);
    Ok(())
}
