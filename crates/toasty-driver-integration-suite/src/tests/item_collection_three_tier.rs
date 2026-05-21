//! Three-tier item-collection: Tenant -> User -> Todo
//!
//! The 2-tier suite in `item_collection.rs` uses single-field FKs throughout
//! (Todo.user_id -> User.id). The 3-tier shape forces composite FKs on every
//! level: User has FK (tenant_id) -> Tenant.id, Todo has FK (tenant_id,
//! user_id) -> User PK. This catches engine paths the 2-tier shape never
//! exercises:
//!
//!   - composite-FK lowering (eq/ne operand rewrite, IN-subquery lift)
//!   - composite-FK index_match against the shared SK prefix
//!   - nested_merge qualifiers when the child FK is a record, not a scalar
//!   - multi-level .include() chains

use crate::prelude::*;

#[derive(Debug, toasty::Model)]
struct Tenant {
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,

    #[has_many]
    users: toasty::HasMany<User>,
}

#[derive(Debug, toasty::Model)]
#[item_collection(Tenant)]
#[key(partition = tenant_id, local = id)]
struct User {
    id: String,

    tenant_id: uuid::Uuid,

    #[belongs_to(key = tenant_id, references = id)]
    tenant: toasty::BelongsTo<Tenant>,

    name: String,

    #[has_many]
    todos: toasty::HasMany<Todo>,
}

#[derive(Debug, toasty::Model)]
#[item_collection(User)]
#[key(partition = tenant_id, local = [user_id, id])]
#[index(tenant_id, user_id)]
struct Todo {
    id: String,

    tenant_id: uuid::Uuid,
    user_id: String,

    #[belongs_to(key = [tenant_id, user_id], references = [tenant_id, id])]
    user: toasty::BelongsTo<User>,

    title: String,
}

async fn setup(test: &mut Test) -> toasty::Db {
    test.setup_db(models!(Tenant, User, Todo)).await
}

/// Schema build round-trips: registering Tenant, User, Todo with their
/// composite-FK relations and chained item_collection annotations is
/// accepted by the schema validator.
#[driver_test(requires(native_starts_with))]
pub async fn three_tier_setup(test: &mut Test) -> Result<()> {
    let _db = setup(test).await;
    Ok(())
}

/// Create a Tenant, a scoped User under it, and a scoped Todo under that
/// User. Verify each round-trips by primary key. Exercises the insert
/// path for composite-FK child models at two depths.
#[driver_test(requires(native_starts_with))]
pub async fn three_tier_create_each_tier(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant { name: "Acme" })
        .exec(&mut db)
        .await?;

    let alice = toasty::create!(in acme.users() {
        id: "alice".to_string(),
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    let todo = toasty::create!(in alice.todos() {
        id: "t1".to_string(),
        title: "ship it",
    })
    .exec(&mut db)
    .await?;

    let reloaded_user =
        User::get_by_tenant_id_and_id(&mut db, &alice.tenant_id, &alice.id).await?;
    assert_eq!(reloaded_user.name, "Alice");

    let reloaded_todo = Todo::get_by_tenant_id_and_user_id_and_id(
        &mut db,
        &todo.tenant_id,
        &todo.user_id,
        &todo.id,
    )
    .await?;
    assert_eq!(reloaded_todo.title, "ship it");

    Ok(())
}

/// Read each tier through its parent's scoped relation. Exercises the
/// query-planning path that translates a parent reference into a
/// composite-FK + SK-prefix DDB query.
#[driver_test(requires(native_starts_with))]
pub async fn three_tier_scoped_reads(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant { name: "Acme" })
        .exec(&mut db)
        .await?;

    let alice = toasty::create!(in acme.users() {
        id: "alice".to_string(),
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    let _bob = toasty::create!(in acme.users() {
        id: "bob".to_string(),
        name: "Bob",
    })
    .exec(&mut db)
    .await?;

    for i in 0..3 {
        toasty::create!(in alice.todos() {
            id: format!("t{i}"),
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

/// Filter by partial composite key on the deepest model. Exercises the
/// composite-FK index match: (tenant_id, user_id) is a prefix of the
/// PK, so the planner should drive a single-partition query instead of
/// a scan.
#[driver_test(requires(native_starts_with))]
pub async fn three_tier_filter_by_partial_composite_key(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant { name: "Acme" })
        .exec(&mut db)
        .await?;

    let alice = toasty::create!(in acme.users() {
        id: "alice".to_string(),
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    for i in 0..3 {
        toasty::create!(in alice.todos() {
            id: format!("t{i}"),
            title: format!("todo {i}"),
        })
        .exec(&mut db)
        .await?;
    }

    let todos = Todo::filter_by_tenant_id(acme.id)
        .filter_by_user_id(&alice.id)
        .exec(&mut db)
        .await?;
    assert_eq!(3, todos.len());

    Ok(())
}

/// Include the deepest tier's children on a middle-tier query. The
/// User query supplies (tenant_id, id) as the composite PK; the include
/// subquery joins Todo through that composite key. This is the path
/// the example exercises and the original failure mode that drove the
/// IsModel work.
#[driver_test(requires(native_starts_with))]
pub async fn three_tier_include_deepest(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant { name: "Acme" })
        .exec(&mut db)
        .await?;

    let alice = toasty::create!(in acme.users() {
        id: "alice".to_string(),
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    for i in 0..3 {
        toasty::create!(in alice.todos() {
            id: format!("t{i}"),
            title: format!("todo {i}"),
        })
        .exec(&mut db)
        .await?;
    }

    let mut users = User::filter_by_tenant_id(acme.id)
        .filter_by_id(&alice.id)
        .include(User::fields().todos())
        .exec(&mut db)
        .await?;
    assert_eq!(1, users.len());
    let from_db = users.pop().unwrap();
    assert_eq!(3, from_db.todos.get().len());

    Ok(())
}

/// Update the deepest tier through a chained scope. Exercises the
/// update path against a composite-PK shared table.
#[driver_test(requires(native_starts_with))]
pub async fn three_tier_scoped_update(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant { name: "Acme" })
        .exec(&mut db)
        .await?;

    let alice = toasty::create!(in acme.users() {
        id: "alice".to_string(),
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    let todo = toasty::create!(in alice.todos() {
        id: "t1".to_string(),
        title: "before",
    })
    .exec(&mut db)
    .await?;

    alice
        .todos()
        .filter_by_id(&todo.id)
        .update()
        .title("after")
        .exec(&mut db)
        .await?;

    let reloaded = Todo::get_by_tenant_id_and_user_id_and_id(
        &mut db,
        &todo.tenant_id,
        &todo.user_id,
        &todo.id,
    )
    .await?;
    assert_eq!(reloaded.title, "after");

    Ok(())
}

/// Batch-create children at the deepest tier. Each row's scope is
/// `alice.todos()` — the same parent Scope target — so `Insert::merge`
/// must collapse them into one VALUES list. The composite FK (tenant_id,
/// user_id) on Todo is what stresses the merge path beyond the 2-tier case.
#[driver_test(requires(native_starts_with))]
pub async fn three_tier_batch_create_deepest(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant { name: "Acme" })
        .exec(&mut db)
        .await?;

    let alice = toasty::create!(in acme.users() {
        id: "alice".to_string(),
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    let mut builder = Todo::create_many();
    for i in 0..5 {
        builder = builder.item(toasty::create!(in alice.todos() {
            id: format!("t{i}"),
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

/// Batch-create middle-tier children under a Tenant. Single-FK scope
/// (tenant_id only). Confirms the 2-tier merge fix continues to work
/// when the Scope target is one level deep, even though the schema also
/// has a deeper tier defined.
#[driver_test(requires(native_starts_with))]
pub async fn three_tier_batch_create_middle(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant { name: "Acme" })
        .exec(&mut db)
        .await?;

    let mut builder = User::create_many();
    for name in ["Alice", "Bob", "Carol"] {
        builder = builder.item(toasty::create!(in acme.users() {
            id: name.to_lowercase(),
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
#[driver_test(requires(native_starts_with))]
pub async fn three_tier_cascade_delete(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let acme = toasty::create!(Tenant { name: "Acme" })
        .exec(&mut db)
        .await?;

    let alice = toasty::create!(in acme.users() {
        id: "alice".to_string(),
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    let todo = toasty::create!(in alice.todos() {
        id: "t1".to_string(),
        title: "doomed",
    })
    .exec(&mut db)
    .await?;

    let tenant_id = acme.id;
    let alice_tenant_id = alice.tenant_id;
    let alice_id = alice.id.clone();
    let todo_tenant_id = todo.tenant_id;
    let todo_user_id = todo.user_id.clone();
    let todo_id = todo.id.clone();

    acme.delete().exec(&mut db).await?;

    assert_err!(Tenant::get_by_id(&mut db, &tenant_id).await);
    assert_err!(User::get_by_tenant_id_and_id(&mut db, &alice_tenant_id, &alice_id).await);
    assert_err!(
        Todo::get_by_tenant_id_and_user_id_and_id(
            &mut db,
            &todo_tenant_id,
            &todo_user_id,
            &todo_id,
        )
        .await
    );

    Ok(())
}
