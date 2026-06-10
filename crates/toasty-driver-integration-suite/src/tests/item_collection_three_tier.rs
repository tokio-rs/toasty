//! Three-tier item collection: Tenant -> User -> Todo.
//!
//! All three models share `#[key(account, sk)]`; descendants declare
//! `#[item_parent]` toward their immediate parent, and Toasty mints
//! `sk` hierarchically: `Tenant#`, `User#<u-uuid>`, `Todo#<u-uuid>#<own>`.
//!
//! These tests exercise the read/write/cascade paths across two
//! `HasItems` hops (Tenant.users -> User.todos), the deep-chain partition
//! + sort-prefix planning, and parent-deletion cascade through both tiers.

use crate::prelude::*;

#[derive(Debug, toasty::Model)]
#[key(account, sk)]
struct Tenant {
    account: String,

    #[auto]
    sk: String,

    name: String,

    #[has_many]
    users: toasty::Deferred<Vec<User>>,
}

#[derive(Debug, toasty::Model)]
#[key(account, sk)]
struct User {
    account: String,

    #[auto]
    sk: String,

    name: String,

    #[item_parent]
    tenant: toasty::Deferred<Tenant>,

    #[has_many]
    todos: toasty::Deferred<Vec<Todo>>,
}

#[derive(Debug, toasty::Model)]
#[key(account, sk)]
struct Todo {
    account: String,

    #[auto]
    sk: String,

    title: String,

    #[item_parent]
    user: toasty::Deferred<User>,
}

async fn setup(test: &mut Test) -> toasty::Db {
    test.setup_db(models!(Tenant, User, Todo)).await
}

/// Schema build round-trips: registering Tenant, User, Todo with their
/// `#[item_parent]` chain and shared `#[key(account, sk)]` is accepted
/// by the schema validator.
#[driver_test(requires(not(sql)))]
pub async fn three_tier_setup(test: &mut Test) -> Result<()> {
    let _db = setup(test).await;
    Ok(())
}

/// Create a Tenant, a scoped User under it, and a scoped Todo under that
/// User. Verify each round-trips by primary key. Exercises the insert
/// path across two `#[item_parent]` hops.
#[driver_test(requires(not(sql)))]
pub async fn three_tier_create_each_tier(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant {
        account: "acme",
        name: "Acme"
    })
    .exec(&mut db)
    .await?;

    let alice = toasty::create!(in acme.users() {
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    let todo = toasty::create!(in alice.todos() {
        title: "ship it",
    })
    .exec(&mut db)
    .await?;

    let reloaded_user = User::get_by_account_and_sk(&mut db, &alice.account, &alice.sk).await?;
    assert_eq!(reloaded_user.name, "Alice");

    let reloaded_todo = Todo::get_by_account_and_sk(&mut db, &todo.account, &todo.sk).await?;
    assert_eq!(reloaded_todo.title, "ship it");

    Ok(())
}

/// Read each tier through its parent's scoped relation. Exercises the
/// query-planning path that translates a parent reference into a
/// partition + sort-prefix DDB query.
#[driver_test(requires(not(sql)))]
pub async fn three_tier_scoped_reads(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant {
        account: "acme",
        name: "Acme"
    })
    .exec(&mut db)
    .await?;

    let alice = toasty::create!(in acme.users() {
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    let _bob = toasty::create!(in acme.users() {
        name: "Bob",
    })
    .exec(&mut db)
    .await?;

    for i in 0..3 {
        toasty::create!(in alice.todos() {
            title: format!("todo {i}"),
        })
        .exec(&mut db)
        .await?;
    }

    let users = acme.users().exec(&mut db).await?;
    assert_eq!(2, users.len());

    let todos = alice.todos().exec(&mut db).await?;
    assert_eq!(3, todos.len());

    Ok(())
}

/// Include the deepest tier's children on a middle-tier query. Exercises
/// `.include()` over a `HasItems` relation in a deep chain — the path
/// the example exercises end-to-end.
#[driver_test(requires(not(sql)))]
#[ignore = "deferred: .include() HasItems over-fetches in deep chains; tracked in plan Follow-ups (B4.11 review): \"Pin the .include() over-fetch as an #[ignore]'d integration test\""]
pub async fn three_tier_include_deepest(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant {
        account: "acme",
        name: "Acme"
    })
    .exec(&mut db)
    .await?;

    let alice = toasty::create!(in acme.users() {
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    for i in 0..3 {
        toasty::create!(in alice.todos() {
            title: format!("todo {i}"),
        })
        .exec(&mut db)
        .await?;
    }

    let mut users = User::filter_by_account(&alice.account)
        .filter_by_sk(alice.sk.clone())
        .include(User::fields().todos())
        .exec(&mut db)
        .await?;
    assert_eq!(1, users.len());
    let from_db = users.pop().unwrap();
    assert_eq!(3, from_db.todos.get().len());

    Ok(())
}

/// Update the deepest tier through a chained scope. Exercises the
/// update path against a shared-table composite PK.
#[driver_test(requires(not(sql)))]
pub async fn three_tier_scoped_update(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant {
        account: "acme",
        name: "Acme"
    })
    .exec(&mut db)
    .await?;

    let alice = toasty::create!(in acme.users() {
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    let todo = toasty::create!(in alice.todos() {
        title: "before",
    })
    .exec(&mut db)
    .await?;

    alice
        .todos()
        .filter_by_sk(todo.sk.clone())
        .update()
        .title("after")
        .exec(&mut db)
        .await?;

    let reloaded = Todo::get_by_account_and_sk(&mut db, &todo.account, &todo.sk).await?;
    assert_eq!(reloaded.title, "after");

    Ok(())
}

/// Filter-driven update across multiple Todos under one user.
/// The filter matches more than one Todo; all matching rows must be
/// updated, non-matching rows must remain unchanged. Exercises the
/// filter-mutation path at the deepest tier.
#[driver_test(requires(not(sql)))]
pub async fn three_tier_update_many_deepest_by_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant {
        account: "acme",
        name: "Acme"
    })
    .exec(&mut db)
    .await?;

    let alice = toasty::create!(in acme.users() {
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    for label in ["match", "match", "match", "skip"] {
        toasty::create!(in alice.todos() {
            title: label.to_string(),
        })
        .exec(&mut db)
        .await?;
    }

    alice
        .todos()
        .filter(Todo::fields().title().eq("match"))
        .update()
        .title("updated")
        .exec(&mut db)
        .await?;

    let mut titles: Vec<String> = alice
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    titles.sort();
    assert_eq!(titles, vec!["skip", "updated", "updated", "updated"],);

    Ok(())
}

/// Filter-driven delete across multiple Todos under one user.
/// The filter matches more than one Todo; all matching rows must be
/// deleted, non-matching rows must survive. Exercises the filter-mutation
/// path at the deepest tier.
#[driver_test(requires(not(sql)))]
pub async fn three_tier_delete_many_deepest_by_filter(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant {
        account: "acme",
        name: "Acme"
    })
    .exec(&mut db)
    .await?;

    let alice = toasty::create!(in acme.users() {
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    for label in ["doomed", "doomed", "doomed", "survivor"] {
        toasty::create!(in alice.todos() {
            title: label.to_string(),
        })
        .exec(&mut db)
        .await?;
    }

    alice
        .todos()
        .filter(Todo::fields().title().eq("doomed"))
        .delete()
        .exec(&mut db)
        .await?;

    let titles: Vec<String> = alice
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(titles, vec!["survivor".to_string()]);

    Ok(())
}

/// Batch-create children at the deepest tier. Each row's scope is
/// `alice.todos()` -- the same parent Scope target -- so `Insert::merge`
/// must collapse them into one VALUES list. Stresses the merge path
/// across two `#[item_parent]` hops.
#[driver_test(requires(not(sql)))]
pub async fn three_tier_batch_create_deepest(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant {
        account: "acme",
        name: "Acme"
    })
    .exec(&mut db)
    .await?;

    let alice = toasty::create!(in acme.users() {
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    let mut builder = Todo::create_many();
    for i in 0..5 {
        builder = builder.item(toasty::create!(in alice.todos() {
            title: format!("todo {i}"),
        }));
    }
    let todos = builder.exec(&mut db).await?;
    assert_eq!(5, todos.len());

    let mut titles: Vec<String> = alice
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    titles.sort();
    assert_eq!(
        titles,
        vec!["todo 0", "todo 1", "todo 2", "todo 3", "todo 4"]
    );

    Ok(())
}

/// Batch-create middle-tier children under a Tenant. Single `#[item_parent]`
/// hop. Confirms the merge fix continues to work when the Scope target
/// is one level deep, even though the schema also has a deeper tier.
#[driver_test(requires(not(sql)))]
pub async fn three_tier_batch_create_middle(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant {
        account: "acme",
        name: "Acme"
    })
    .exec(&mut db)
    .await?;

    let mut builder = User::create_many();
    for name in ["Alice", "Bob", "Carol"] {
        builder = builder.item(toasty::create!(in acme.users() {
            name: name,
        }));
    }
    let users = builder.exec(&mut db).await?;
    assert_eq!(3, users.len());

    let mut names: Vec<String> = acme
        .users()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|u| u.name)
        .collect();
    names.sort();
    assert_eq!(names, vec!["Alice", "Bob", "Carol"]);

    Ok(())
}

/// Cascade delete from the root: deleting a Tenant removes its Users
/// and their Todos. Verifies the cascade chain reaches two levels deep.
#[driver_test(requires(not(sql)))]
pub async fn three_tier_cascade_delete(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant {
        account: "acme",
        name: "Acme"
    })
    .exec(&mut db)
    .await?;

    let alice = toasty::create!(in acme.users() {
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    let todo = toasty::create!(in alice.todos() {
        title: "doomed",
    })
    .exec(&mut db)
    .await?;

    let tenant_account = acme.account.clone();
    let tenant_sk = acme.sk.clone();
    let alice_account = alice.account.clone();
    let alice_sk = alice.sk.clone();
    let todo_account = todo.account.clone();
    let todo_sk = todo.sk.clone();

    acme.delete().exec(&mut db).await?;

    assert_err!(Tenant::get_by_account_and_sk(&mut db, &tenant_account, &tenant_sk).await);
    assert_err!(User::get_by_account_and_sk(&mut db, &alice_account, &alice_sk).await);
    assert_err!(Todo::get_by_account_and_sk(&mut db, &todo_account, &todo_sk).await);

    Ok(())
}
