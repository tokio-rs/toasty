//! Test basic has_many associations without any preloading of associations
//! during query time. All associations are accessed via queries on demand.

use crate::prelude::*;
use hashbrown::HashMap;

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn crud_user_todos(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    // Create a user
    let user = User::create().name("User 1").exec(&mut db).await?;

    // No TODOs
    assert_eq!(0, user.todos().exec(&mut db).await?.len());

    // Create a Todo associated with the user
    let todo = user
        .todos()
        .create()
        .title("hello world")
        .exec(&mut db)
        .await?;

    // Find the todo by ID
    let list = Todo::filter_by_id(todo.id).exec(&mut db).await?;

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    // Find the TODO by user ID
    let list = Todo::filter_by_user_id(user.id).exec(&mut db).await?;

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    // Find the User using the Todo
    let user_reload = User::get_by_id(&mut db, &todo.user_id).await?;
    assert_eq!(user.id, user_reload.id);

    let mut created = HashMap::new();
    let mut ids = vec![todo.id];
    created.insert(todo.id, todo);

    // Create a few more TODOs
    for i in 0..5 {
        let title = format!("hello world {i}");

        let todo = if i.is_even() {
            // Create via user
            user.todos().create().title(title).exec(&mut db).await?
        } else {
            // Create via todo builder
            Todo::create()
                .user(&user)
                .title(title)
                .exec(&mut db)
                .await?
        };

        ids.push(todo.id);
        assert_none!(created.insert(todo.id, todo));
    }

    // Load all TODOs
    let list = user.todos().exec(&mut db).await?;

    assert_eq!(6, list.len());

    let loaded: HashMap<_, _> = list.into_iter().map(|todo| (todo.id, todo)).collect();
    assert_eq!(6, loaded.len());

    for (id, expect) in &created {
        assert_eq!(expect.title, loaded[id].title);
    }

    // Find all TODOs by user (using the belongs_to queries)
    let list = Todo::filter_by_user_id(user.id).exec(&mut db).await?;
    assert_eq!(6, list.len());

    let by_id: HashMap<_, _> = list.into_iter().map(|todo| (todo.id, todo)).collect();

    assert_eq!(6, by_id.len());

    for (id, expect) in by_id {
        assert_eq!(expect.title, loaded[&id].title);
    }

    // Create a second user
    let user2 = User::create().name("User 2").exec(&mut db).await?;

    // No TODOs associated with `user2`
    assert_eq!(0, user2.todos().exec(&mut db).await?.len());

    // Create a TODO for user2
    let u2_todo = user2
        .todos()
        .create()
        .title("user 2 todo")
        .exec(&mut db)
        .await?;

    {
        let u1_todos = user.todos().exec(&mut db).await?;

        for todo in u1_todos {
            assert_ne!(u2_todo.id, todo.id);
        }
    }

    // Delete a TODO by value
    let todo = Todo::get_by_id(&mut db, &ids[0]).await?;
    todo.delete().exec(&mut db).await?;

    // Can no longer get the todo via id
    assert_err!(Todo::get_by_id(&mut db, &ids[0]).await);

    // Can no longer get the todo scoped
    assert_err!(user.todos().get_by_id(&mut db, &ids[0]).await);

    // Delete a TODO by scope
    user.todos()
        .filter_by_id(ids[1])
        .delete()
        .exec(&mut db)
        .await?;

    // Can no longer get the todo via id
    assert_err!(Todo::get_by_id(&mut db, &ids[1]).await);

    // Can no longer get the todo scoped
    assert_err!(user.todos().get_by_id(&mut db, &ids[1]).await);

    // Successfuly a todo by scope
    user.todos()
        .filter_by_id(ids[2])
        .update()
        .title("batch update 1")
        .exec(&mut db)
        .await?;

    let todo = Todo::get_by_id(&mut db, &ids[2]).await?;
    assert_eq!(todo.title, "batch update 1");

    // Now fail to update it by scoping by other user
    user2
        .todos()
        .filter_by_id(ids[2])
        .update()
        .title("batch update 2")
        .exec(&mut db)
        .await?;

    let todo = Todo::get_by_id(&mut db, &ids[2]).await?;
    assert_eq!(todo.title, "batch update 1");

    let id = user.id;

    // Delete the user and associated TODOs are deleted
    user.delete().exec(&mut db).await?;
    assert_err!(User::get_by_id(&mut db, &id).await);
    assert_err!(Todo::get_by_id(&mut db, &ids[2]).await);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn has_many_insert_on_update(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    // Create a user, no TODOs
    let mut user = User::create().name("Alice").exec(&mut db).await?;
    assert!(user.todos().exec(&mut db).await?.is_empty());

    // Update the user and create a todo in a batch
    user.update()
        .name("Bob")
        .todos(toasty::stmt::insert(Todo::create().title("change name")))
        .exec(&mut db)
        .await?;

    assert_eq!("Bob", user.name);
    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    assert_eq!(todos[0].title, "change name");
    Ok(())
}

/// `stmt::apply([])` on a has-many is a no-op: the surface API's empty
/// Apply loop adds no entry to the assignments map, so the relation
/// field is treated as unchanged. Run alongside a separate scalar
/// change because the engine verifier rejects updates with no
/// assignments at all.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn has_many_apply_empty_is_noop(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Alice").exec(&mut db).await?;
    user.todos()
        .create()
        .title("existing")
        .exec(&mut db)
        .await?;

    user.update()
        .name("Bob")
        .todos(toasty::stmt::apply::<toasty::stmt::List<Todo>>([]))
        .exec(&mut db)
        .await?;

    assert_eq!(user.name, "Bob");
    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].title, "existing");
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn has_many_apply_multiple_inserts(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Alice").exec(&mut db).await?;

    user.update()
        .todos(toasty::stmt::apply([
            toasty::stmt::insert(Todo::create().title("Buy groceries")),
            toasty::stmt::insert(Todo::create().title("Walk the dog")),
        ]))
        .exec(&mut db)
        .await?;

    let mut titles: Vec<_> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    titles.sort();
    assert_eq!(titles, ["Buy groceries", "Walk the dog"]);
    Ok(())
}

/// Sanity check for plain `update().todos(stmt::remove(..))` — no
/// `apply` involved. With a required FK, Remove deletes the child row.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn has_many_update_remove(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Alice").exec(&mut db).await?;
    let old_todo = user.todos().create().title("old").exec(&mut db).await?;

    user.update()
        .todos(toasty::stmt::remove(&old_todo))
        .exec(&mut db)
        .await?;

    assert_eq!(0, user.todos().exec(&mut db).await?.len());
    Ok(())
}

/// `stmt::apply([insert(..), remove(..)])` mixes Insert and Remove on a
/// has-many in one update. Each entry dispatches as its own Mutation:
/// the Insert associates the new child; the Remove dissociates the old
/// one (and for a required FK, deletes it).
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn has_many_apply_insert_and_remove(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Alice").exec(&mut db).await?;
    let old_todo = user.todos().create().title("old").exec(&mut db).await?;

    user.update()
        .todos(toasty::stmt::apply([
            toasty::stmt::insert(Todo::create().title("new")),
            toasty::stmt::remove(&old_todo),
        ]))
        .exec(&mut db)
        .await?;

    let titles: Vec<_> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(titles, ["new"]);
    Ok(())
}

/// `stmt::apply([remove(..), insert(..)])` — the reverse of
/// `has_many_apply_insert_and_remove`. `flatten_relation_batch` always
/// emits the merged Insert first, so the final state is order-independent:
/// the new child is associated and the old one is removed.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn has_many_apply_remove_then_insert(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Alice").exec(&mut db).await?;
    let old_todo = user.todos().create().title("old").exec(&mut db).await?;

    user.update()
        .todos(toasty::stmt::apply([
            toasty::stmt::remove(&old_todo),
            toasty::stmt::insert(Todo::create().title("new")),
        ]))
        .exec(&mut db)
        .await?;

    let titles: Vec<_> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(titles, ["new"]);
    Ok(())
}

/// `stmt::apply([insert(a), insert(b), remove(c)])` — multiple inserts
/// merge into one multi-row INSERT, dispatched alongside a separate
/// Remove. Exercises the Insert-merge path plus a sibling disassociate.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn has_many_apply_two_inserts_and_remove(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Alice").exec(&mut db).await?;
    let old_todo = user.todos().create().title("old").exec(&mut db).await?;

    user.update()
        .todos(toasty::stmt::apply([
            toasty::stmt::insert(Todo::create().title("a")),
            toasty::stmt::insert(Todo::create().title("b")),
            toasty::stmt::remove(&old_todo),
        ]))
        .exec(&mut db)
        .await?;

    let mut titles: Vec<_> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    titles.sort();
    assert_eq!(titles, ["a", "b"]);
    Ok(())
}

/// `stmt::apply([remove(a), remove(b)])` — only disassociations, no
/// Insert. `flatten_relation_batch` pushes no merged Insert, so both
/// entries dispatch as standalone Disassociate mutations.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn has_many_apply_multiple_removes(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Alice").exec(&mut db).await?;
    let t1 = user.todos().create().title("t1").exec(&mut db).await?;
    let t2 = user.todos().create().title("t2").exec(&mut db).await?;
    let t3 = user.todos().create().title("keep").exec(&mut db).await?;

    user.update()
        .todos(toasty::stmt::apply([
            toasty::stmt::remove(&t1),
            toasty::stmt::remove(&t2),
        ]))
        .exec(&mut db)
        .await?;

    let titles: Vec<_> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(titles, ["keep"]);

    // Required FK: removed todos are deleted, not just unlinked.
    assert_err!(Todo::get_by_id(&mut db, &t1.id).await);
    assert_err!(Todo::get_by_id(&mut db, &t2.id).await);
    assert_ok!(Todo::get_by_id(&mut db, &t3.id).await);
    Ok(())
}

/// `stmt::apply([insert(..), remove(..)])` on a has-many with a *nullable*
/// foreign key. Unlike the required-FK case (which deletes the child),
/// Remove here takes the disassociate-nullify branch: the old todo
/// persists with its FK set to NULL.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_nullable_fk))]
pub async fn has_many_apply_insert_and_remove_nullable_fk(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().exec(&mut db).await?;
    let old_todo = user.todos().create().title("old").exec(&mut db).await?;

    user.update()
        .todos(toasty::stmt::apply([
            toasty::stmt::insert(Todo::create().title("new")),
            toasty::stmt::remove(&old_todo),
        ]))
        .exec(&mut db)
        .await?;

    let titles: Vec<_> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(titles, ["new"]);

    // Nullable FK: the removed todo is unlinked, not deleted.
    let reloaded = Todo::get_by_id(&mut db, &old_todo.id).await?;
    assert_none!(reloaded.user_id);
    Ok(())
}

/// Order-sensitive swap: the child has a `#[unique]` title and we replace
/// the "X" todo by removing the old one and inserting a fresh "X". With a
/// required FK, `remove` deletes the old row (freeing the unique title), so
/// the insert can reuse it — but only if the delete runs first.
///
/// `flatten_relation_batch` dispatches the merged Insert after the batch's
/// removes, so the delete lands before the insert and the swap succeeds
/// regardless of the order the caller wrote the entries.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_unique_title))]
pub async fn has_many_apply_swap_unique_required_fk(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().exec(&mut db).await?;
    let old = user.todos().create().title("X").exec(&mut db).await?;

    user.update()
        .todos(toasty::stmt::apply([
            toasty::stmt::remove(&old),
            toasty::stmt::insert(Todo::create().title("X")),
        ]))
        .exec(&mut db)
        .await?;

    let titles: Vec<_> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(titles, ["X"]);
    Ok(())
}

/// Same unique-title swap, but with an unrelated `insert` at the *front* of
/// the batch. `flatten_relation_batch` merges all inserts into one multi-row
/// INSERT, so the unrelated "Y" insert and the swap's new "X" insert become a
/// single statement. That merged INSERT must still be dispatched after the
/// `remove`, or the new "X" collides with the old one on the unique
/// constraint — i.e. coalescing inserts must not pull them ahead of removes.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_unique_title))]
pub async fn has_many_apply_swap_unique_with_extra_insert(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().exec(&mut db).await?;
    let old = user.todos().create().title("X").exec(&mut db).await?;

    user.update()
        .todos(toasty::stmt::apply([
            toasty::stmt::insert(Todo::create().title("Y")),
            toasty::stmt::remove(&old),
            toasty::stmt::insert(Todo::create().title("X")),
        ]))
        .exec(&mut db)
        .await?;

    let mut titles: Vec<_> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    titles.sort();
    assert_eq!(titles, ["X", "Y"]);
    Ok(())
}

/// Inserting and removing the *same existing* record in one batch honors
/// entry order. `insert(&t)` (associate an existing row) and `remove(&t)`
/// (dissociate it) both lower to UPDATEs on the same row; the batch sequences
/// its entries so the last-written op wins, instead of the two UPDATEs racing
/// in the dependency graph.
///
/// Note this is orthogonal to `flatten_relation_batch`'s insert-last reorder —
/// that only moves create-new inserts (`Todo::create()`), not
/// associate-existing inserts (`&todo`), which keep their written position.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_nullable_fk))]
pub async fn has_many_apply_insert_remove_same_item(test: &mut Test) -> Result<()> {
    use toasty_core::{
        driver::Operation,
        stmt::{Assignment, ExprSet, Statement, Update},
    };

    // Drain the op log into one marker per FK-writing UPDATE: "unlink" for
    // `Set(NULL)` (dissociate), "link" otherwise (associate). Transaction,
    // savepoint, and read (COUNT/EXISTS) ops are ignored. SQL drivers only —
    // key-value drivers emit a different op shape.
    fn fk_writes(test: &Test) -> Vec<&'static str> {
        fn classify(update: &Update, out: &mut Vec<&'static str>) {
            for (_, assignment) in update.assignments.iter() {
                if let Assignment::Set(expr) = assignment {
                    out.push(if expr.is_value_null() {
                        "unlink"
                    } else {
                        "link"
                    });
                }
            }
        }

        let mut out = vec![];
        while !test.log().is_empty() {
            let Operation::QuerySql(q) = test.log().pop().0 else {
                continue;
            };
            match &q.stmt {
                // The associate update, plus the dissociate's write half on
                // drivers without `cte_with_update` (its conditional update
                // lowers to a read-modify-write with a bare UPDATE).
                Statement::Update(update) => classify(update, &mut out),
                // On a `cte_with_update` driver (e.g. PostgreSQL), the
                // conditional dissociate folds its count check and UPDATE into
                // a single CTE query; the UPDATE lives in a `With` CTE.
                Statement::Query(query) => {
                    for cte in query.with.iter().flat_map(|with| &with.ctes) {
                        if let ExprSet::Update(update) = &cte.query.body {
                            classify(update, &mut out);
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    let mut db = setup(test).await;

    // remove then insert → insert wins → still associated.
    let mut keep = User::create().exec(&mut db).await?;
    let kt = keep.todos().create().title("t").exec(&mut db).await?;
    test.log().clear();
    keep.update()
        .todos(toasty::stmt::apply([
            toasty::stmt::remove(&kt),
            toasty::stmt::insert(&kt),
        ]))
        .exec(&mut db)
        .await?;
    if test.capability().sql {
        // Dissociate executes before associate.
        assert_eq!(fk_writes(test), ["unlink", "link"]);
    }
    assert_eq!(keep.todos().exec(&mut db).await?.len(), 1);

    // insert then remove → remove wins → dissociated.
    let mut drop = User::create().exec(&mut db).await?;
    let dt = drop.todos().create().title("t").exec(&mut db).await?;
    test.log().clear();
    drop.update()
        .todos(toasty::stmt::apply([
            toasty::stmt::insert(&dt),
            toasty::stmt::remove(&dt),
        ]))
        .exec(&mut db)
        .await?;
    if test.capability().sql {
        // Associate executes before dissociate.
        assert_eq!(fk_writes(test), ["link", "unlink"]);
    }
    assert_eq!(drop.todos().exec(&mut db).await?.len(), 0);
    Ok(())
}

/// Nested `stmt::apply([apply([..]), ..])`. The surface API flattens
/// nested applies into a single flat `Batch` (the engine never sees
/// nesting), so this must behave identically to the equivalent flat
/// batch. Guards the flattening contract — if nesting ever stopped
/// flattening, the engine's `flatten_relation_batch` dispatch would hit
/// its `unreachable!` arm.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn has_many_apply_nested(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Alice").exec(&mut db).await?;
    let old1 = user.todos().create().title("old1").exec(&mut db).await?;
    let old2 = user.todos().create().title("old2").exec(&mut db).await?;

    user.update()
        .todos(toasty::stmt::apply([
            toasty::stmt::apply([
                toasty::stmt::insert(Todo::create().title("a")),
                toasty::stmt::insert(Todo::create().title("b")),
            ]),
            toasty::stmt::apply([toasty::stmt::remove(&old1), toasty::stmt::remove(&old2)]),
            toasty::stmt::insert(Todo::create().title("c")),
        ]))
        .exec(&mut db)
        .await?;

    let mut titles: Vec<_> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    titles.sort();
    assert_eq!(titles, ["a", "b", "c"]);
    Ok(())
}

/// Deterministic matrix over batch shapes. For each
/// `(existing, insert, remove)` combination, build one
/// `stmt::apply([...])` carrying `insert` new children plus `remove`
/// dissociations, then compare the resulting association set against a
/// reference computed in memory. This covers larger batches and more
/// combinations than the targeted tests above without the
/// non-determinism (and sync/async friction) of a `proptest` runner.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn has_many_apply_combinations(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    for num_existing in 0..=3usize {
        for num_insert in 0..=3usize {
            for num_remove in 0..=num_existing {
                // An empty batch produces no assignments, which the engine
                // rejects as an empty update. Covered by
                // `has_many_apply_empty_is_noop`.
                if num_insert == 0 && num_remove == 0 {
                    continue;
                }

                let mut user = User::create()
                    .name(format!("u-{num_existing}-{num_insert}-{num_remove}"))
                    .exec(&mut db)
                    .await?;

                // Seed existing children: e0..e{num_existing}.
                let mut existing = Vec::new();
                for i in 0..num_existing {
                    existing.push(
                        user.todos()
                            .create()
                            .title(format!("e{i}"))
                            .exec(&mut db)
                            .await?,
                    );
                }

                // One batch: `num_insert` inserts + `num_remove` removes.
                let mut ops: Vec<toasty::stmt::Assignment<toasty::stmt::List<Todo>>> = Vec::new();
                for i in 0..num_insert {
                    ops.push(toasty::stmt::insert(Todo::create().title(format!("n{i}"))));
                }
                for todo in &existing[..num_remove] {
                    ops.push(toasty::stmt::remove(todo));
                }

                user.update()
                    .todos(toasty::stmt::apply(ops))
                    .exec(&mut db)
                    .await?;

                // Reference: surviving existing + inserted.
                let mut expected: Vec<String> = existing[num_remove..]
                    .iter()
                    .map(|t| t.title.clone())
                    .collect();
                for i in 0..num_insert {
                    expected.push(format!("n{i}"));
                }
                expected.sort();

                let mut actual: Vec<String> = user
                    .todos()
                    .exec(&mut db)
                    .await?
                    .into_iter()
                    .map(|t| t.title)
                    .collect();
                actual.sort();

                assert_eq!(
                    actual, expected,
                    "existing={num_existing} insert={num_insert} remove={num_remove}"
                );
            }
        }
    }
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn scoped_find_by_id(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    // Create a couple of users
    let user1 = User::create().name("User 1").exec(&mut db).await?;
    let user2 = User::create().name("User 2").exec(&mut db).await?;

    // Create a todo
    let todo = user1
        .todos()
        .create()
        .title("hello world")
        .exec(&mut db)
        .await?;

    // Find it scoped by user1
    let reloaded = user1.todos().get_by_id(&mut db, &todo.id).await?;
    assert_eq!(reloaded.id, todo.id);
    assert_eq!(reloaded.title, todo.title);

    // Trying to find the same todo scoped by user2 is missing
    assert_none!(
        user2
            .todos()
            .filter_by_id(todo.id)
            .first()
            .exec(&mut db)
            .await?
    );

    let reloaded = User::filter_by_id(user1.id)
        .todos()
        .get_by_id(&mut db, &todo.id)
        .await?;

    assert_eq!(reloaded.id, todo.id);
    assert_eq!(reloaded.title, todo.title);

    // Deleting the TODO from the user 2 scope fails
    user2
        .todos()
        .filter_by_id(todo.id)
        .delete()
        .exec(&mut db)
        .await?;
    let reloaded = user1.todos().get_by_id(&mut db, &todo.id).await?;
    assert_eq!(reloaded.id, todo.id);
    Ok(())
}

// The has_many association uses the target's primary key as the association's
// foreign key. In this case, the relation's query should not be duplicated.
#[driver_test(id(ID))]
pub async fn has_many_on_target_pk(_test: &mut Test) {}

// The target model has an explicit index on (FK, PK). In this case, the query
// generated by the (FK, PK) pair should not be duplicated by the relation.
#[driver_test(id(ID))]
pub async fn has_many_when_target_indexes_fk_and_pk(_test: &mut Test) {}

// When the FK is composite, things should still work
#[driver_test(id(ID))]
pub async fn has_many_when_fk_is_composite(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::Deferred<Vec<Todo>>,
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = user_id, local = id)]
    struct Todo {
        #[auto]
        id: uuid::Uuid,

        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,

        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    // Create a user
    let user = User::create().exec(&mut db).await?;

    // No TODOs
    assert_eq!(0, user.todos().exec(&mut db).await?.len());

    // Create a Todo associated with the user
    let todo = user
        .todos()
        .create()
        .title("hello world")
        .exec(&mut db)
        .await?;

    // Find the todo by ID
    let list = Todo::filter_by_user_id_and_id(user.id, todo.id)
        .exec(&mut db)
        .await?;

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    // Find the TODO by user ID
    let list = Todo::filter_by_user_id(user.id).exec(&mut db).await?;

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    let mut created = HashMap::new();
    let mut ids = vec![todo.id];
    created.insert(todo.id, todo);

    // Create a few more TODOs
    for i in 0..5 {
        let title = format!("hello world {i}");

        let todo = if i.is_even() {
            // Create via user
            user.todos().create().title(title).exec(&mut db).await?
        } else {
            // Create via todo builder
            Todo::create()
                .user(&user)
                .title(title)
                .exec(&mut db)
                .await?
        };

        ids.push(todo.id);
        assert_none!(created.insert(todo.id, todo));
    }

    // Load all TODOs
    let list = user.todos().exec(&mut db).await?;

    assert_eq!(6, list.len());

    let loaded: HashMap<_, _> = list.into_iter().map(|todo| (todo.id, todo)).collect();
    assert_eq!(6, loaded.len());

    for (id, expect) in &created {
        assert_eq!(expect.title, loaded[id].title);
    }

    // Find all TODOs by user (using the belongs_to queries)
    let list = Todo::filter_by_user_id(user.id).exec(&mut db).await?;
    assert_eq!(6, list.len());

    let by_id: HashMap<_, _> = list.into_iter().map(|todo| (todo.id, todo)).collect();

    assert_eq!(6, by_id.len());

    for (id, expect) in by_id {
        assert_eq!(expect.title, loaded[&id].title);
    }

    // Create a second user
    let user2 = User::create().exec(&mut db).await?;

    // No TODOs associated with `user2`
    assert_eq!(0, user2.todos().exec(&mut db).await?.len());

    // Create a TODO for user2
    let u2_todo = user2
        .todos()
        .create()
        .title("user 2 todo")
        .exec(&mut db)
        .await?;

    let u1_todos = user.todos().exec(&mut db).await?;

    for todo in u1_todos {
        assert_ne!(u2_todo.id, todo.id);
    }

    // Delete a TODO by value
    let todo = Todo::get_by_user_id_and_id(&mut db, &user.id, &ids[0]).await?;
    todo.delete().exec(&mut db).await?;

    // Can no longer get the todo via id
    assert_err!(Todo::get_by_user_id_and_id(&mut db, &user.id, &ids[0]).await);

    // Can no longer get the todo scoped
    assert_err!(user.todos().get_by_id(&mut db, &ids[0]).await);

    // Delete a TODO by scope
    user.todos()
        .filter_by_id(ids[1])
        .delete()
        .exec(&mut db)
        .await?;

    // Can no longer get the todo via id
    assert_err!(Todo::get_by_user_id_and_id(&mut db, &user.id, &ids[1]).await);

    // Can no longer get the todo scoped
    assert_err!(user.todos().get_by_id(&mut db, &ids[1]).await);

    // Successfuly a todo by scope
    user.todos()
        .filter_by_id(ids[2])
        .update()
        .title("batch update 1")
        .exec(&mut db)
        .await?;
    let todo = Todo::get_by_user_id_and_id(&mut db, &user.id, &ids[2]).await?;
    assert_eq!(todo.title, "batch update 1");

    // Now fail to update it by scoping by other user
    user2
        .todos()
        .filter_by_id(ids[2])
        .update()
        .title("batch update 2")
        .exec(&mut db)
        .await?;
    let todo = Todo::get_by_user_id_and_id(&mut db, &user.id, &ids[2]).await?;
    assert_eq!(todo.title, "batch update 1");
    Ok(())
}

// When the PK is composite, things should still work
#[driver_test(id(ID))]
pub async fn has_many_when_pk_is_composite(_test: &mut Test) {}

// When both the FK and PK are composite, things should still work
#[driver_test(id(ID))]
pub async fn has_many_when_fk_and_pk_are_composite(_test: &mut Test) {}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn belongs_to_required(test: &mut Test) {
    let mut db = setup(test).await;

    assert_err!(Todo::create().exec(&mut db).await);
}

#[driver_test(id(ID))]
pub async fn delete_when_belongs_to_optional(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::Deferred<Vec<Todo>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<Option<User>>,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    let user = User::create().exec(&mut db).await?;
    let mut ids = vec![];

    for _ in 0..3 {
        let todo = user.todos().create().exec(&mut db).await?;
        ids.push(todo.id);
    }

    // Delete the user
    user.delete().exec(&mut db).await?;

    // All the todos still exist and `user` is set to `None`.
    for id in ids {
        let todo = Todo::get_by_id(&mut db, id).await?;
        assert_none!(todo.user_id);
    }

    // Deleting a user leaves the todo in place.
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn associate_new_user_with_todo_on_update_via_creation(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    // Create a user with a todo
    let u1 = User::create()
        .name("User 1")
        .todo(Todo::create().title("hello world"))
        .exec(&mut db)
        .await?;

    // Get the todo
    let todos: Vec<_> = u1.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    let mut todo = todos.into_iter().next().unwrap();

    todo.update()
        .user(User::create().name("User 2"))
        .exec(&mut db)
        .await?;
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn associate_new_user_with_todo_on_update_query_via_creation(
    test: &mut Test,
) -> Result<()> {
    let mut db = setup(test).await;

    // Create a user with a todo
    let u1 = User::create()
        .name("User 1")
        .todo(Todo::create().title("a todo"))
        .exec(&mut db)
        .await?;

    // Get the todo
    let todos: Vec<_> = u1.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    let todo = todos.into_iter().next().unwrap();

    Todo::filter_by_id(todo.id)
        .update()
        .user(User::create().name("User 2"))
        .exec(&mut db)
        .await?;
    Ok(())
}

#[driver_test(id(ID))]
#[should_panic]
pub async fn update_user_with_null_todo_is_err(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::Deferred<Vec<Todo>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
    }

    use toasty::stmt::{self, IntoExpr};

    let mut db = test.setup_db(models!(User, Todo)).await;

    // Create a user with a todo
    let u1 = User::create().todo(Todo::create()).exec(&mut db).await?;

    // Get the todo
    let todos: Vec<_> = u1.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    let todo = todos.into_iter().next().unwrap();

    // Updating the todo w/ null is an error. Thus requires a bit of a hack to make work
    let mut stmt: stmt::Update<Todo> =
        stmt::Update::new(stmt::Query::from_expr((&todo).into_expr()));
    stmt.set(2, toasty_core::stmt::Value::Null);
    stmt.exec(&mut db).await?;

    // User is not deleted
    let u1_reloaded = User::get_by_id(&mut db, &u1.id).await?;
    assert_eq!(u1_reloaded.id, u1.id);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn assign_todo_that_already_has_user_on_create(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let todo = Todo::create()
        .title("a todo")
        .user(User::create().name("User 1"))
        .exec(&mut db)
        .await?;

    let u1 = todo.user().exec(&mut db).await?;

    let u2 = User::create()
        .name("User 2")
        .todo(&todo)
        .exec(&mut db)
        .await?;

    let todo_reload = Todo::get_by_id(&mut db, &todo.id).await?;

    assert_eq!(u2.id, todo_reload.user_id);

    // First user has no todos
    let todos: Vec<_> = u1.todos().exec(&mut db).await?;
    assert_eq!(0, todos.len());

    // Second user has the todo
    let todos: Vec<_> = u2.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    assert_eq!(todo.id, todos[0].id);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn assign_todo_that_already_has_user_on_update(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let todo = Todo::create()
        .title("a todo")
        .user(User::create().name("User 1"))
        .exec(&mut db)
        .await?;

    let u1 = todo.user().exec(&mut db).await?;

    let mut u2 = User::create().name("User 2").exec(&mut db).await?;

    // Update the user
    u2.update()
        .todos(toasty::stmt::insert(&todo))
        .exec(&mut db)
        .await?;

    let todo_reload = Todo::get_by_id(&mut db, &todo.id).await?;

    assert_eq!(u2.id, todo_reload.user_id);

    // First user has no todos
    let todos: Vec<_> = u1.todos().exec(&mut db).await?;
    assert_eq!(0, todos.len());

    // Second user has the todo
    let todos: Vec<_> = u2.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    assert_eq!(todo.id, todos[0].id);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn assign_existing_user_to_todo(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut todo = Todo::create()
        .title("hello")
        .user(User::create().name("User 1"))
        .exec(&mut db)
        .await?;

    let u1 = todo.user().exec(&mut db).await?;

    let u2 = User::create().name("User 2").exec(&mut db).await?;

    // Update the todo
    todo.update().user(&u2).exec(&mut db).await?;

    let todo_reload = Todo::get_by_id(&mut db, &todo.id).await?;

    assert_eq!(u2.id, todo_reload.user_id);

    // First user has no todos
    let todos: Vec<_> = u1.todos().exec(&mut db).await?;
    assert_eq!(0, todos.len());

    // Second user has the todo
    let todos: Vec<_> = u2.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    assert_eq!(todo.id, todos[0].id);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn assign_todo_to_user_on_update_query(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = User::create().name("User 1").exec(&mut db).await?;

    User::filter_by_id(user.id)
        .update()
        .todos(toasty::stmt::insert(Todo::create().title("hello")))
        .exec(&mut db)
        .await?;

    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    assert_eq!("hello", todos[0].title);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn has_many_when_fk_is_composite_with_snippets(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::Deferred<Vec<Todo>>,
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = user_id, local = id)]
    struct Todo {
        #[auto]
        id: uuid::Uuid,

        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,

        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    // Create users
    let user1 = User::create().exec(&mut db).await?;
    let user2 = User::create().exec(&mut db).await?;

    // Create a Todo associated with the user
    user1
        .todos()
        .create()
        .title("hello world")
        .exec(&mut db)
        .await?;

    let todo2 = user2
        .todos()
        .create()
        .title("hello world")
        .exec(&mut db)
        .await?;

    // Update the Todos with the snippets
    Todo::update_by_user_id(user1.id)
        .title("Title 2")
        .exec(&mut db)
        .await?;

    let todo = Todo::get_by_user_id(&mut db, user1.id).await?;
    assert!(todo.title == "Title 2");

    Todo::update_by_user_id_and_id(user2.id, todo2.id)
        .title("Title 3")
        .exec(&mut db)
        .await?;

    let todo = Todo::get_by_user_id_and_id(&mut db, user2.id, todo2.id).await?;
    assert!(todo.title == "Title 3");

    // Delete the Todos with the snippets
    Todo::delete_by_user_id(&mut db, user1.id).await?;
    assert_err!(Todo::get_by_user_id(&mut db, user1.id).await);

    Todo::delete_by_user_id_and_id(&mut db, user2.id, todo2.id).await?;
    assert_err!(Todo::get_by_user_id_and_id(&mut db, user2.id, todo2.id).await);

    Ok(())
}
