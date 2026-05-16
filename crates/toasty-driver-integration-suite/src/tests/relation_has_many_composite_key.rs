//! Tests for `has_many` / `belongs_to` relationships whose foreign key spans
//! multiple columns. These parallel the single-key relation tests so we can
//! make sure composite-key behavior matches.
//!
//! The shared scenario lives in
//! [`crate::scenarios::composite_has_many_belongs_to`]: a `User` with a
//! single-column auto PK and a `Todo` whose PK is
//! `#[key(partition = user_id, local = id)]`.
//!
//! See: https://github.com/tokio-rs/toasty/discussions/904

use crate::prelude::*;
use hashbrown::HashMap;

/// When a composite-key `belongs_to` has no covering index on the parent
/// side, schema verification must return a helpful invalid-schema error
/// rather than panicking with `failed to find relation index`.
#[driver_test]
pub async fn composite_belongs_to_missing_index_is_error(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id, revision)]
    struct Parent {
        id: String,
        revision: i64,

        #[has_many]
        children: toasty::HasMany<Child>,
    }

    #[derive(Debug, toasty::Model)]
    struct Child {
        #[key]
        id: String,

        // Two field-level `#[index]` annotations create two separate
        // single-column indexes — neither covers the composite foreign key.
        #[index]
        parent_id: String,
        #[index]
        parent_revision: i64,

        #[belongs_to(key = [parent_id, parent_revision], references = [id, revision])]
        parent: toasty::BelongsTo<Parent>,
    }

    let err = test
        .try_setup_db(models!(Parent, Child))
        .await
        .expect_err("schema verification should reject this layout");

    assert!(
        err.is_invalid_schema(),
        "expected invalid_schema error, got: {err}",
    );

    let msg = err.to_string();
    assert!(
        msg.contains("parent_id") && msg.contains("parent_revision"),
        "error should mention the foreign-key fields, got: {msg}",
    );
    assert!(
        msg.contains("#[index(parent_id, parent_revision)]"),
        "error should suggest adding a composite index, got: {msg}",
    );
    // Both FK fields are individually `#[index]`-annotated, so the verifier
    // should detect that and explain why two single-column indexes don't
    // satisfy a composite foreign key.
    assert!(
        msg.contains("each foreign-key field already has its own `#[index]`"),
        "error should call out the per-field `#[index]` annotations, got: {msg}",
    );

    Ok(())
}

// =====================================================================
// Tier A — fundamental CRUD on a composite-FK has_many. These mirror the
// single-key tests in `relation_has_many_crud.rs`.
// =====================================================================

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_crud_user_todos(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = User::create().name("User 1").exec(&mut db).await?;

    assert_eq!(0, user.todos().exec(&mut db).await?.len());

    let todo = user
        .todos()
        .create()
        .title("hello world")
        .exec(&mut db)
        .await?;

    // Find the todo by its composite key
    let list = Todo::filter_by_user_id_and_id(user.id, todo.id)
        .exec(&mut db)
        .await?;
    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    // Find by partition (user_id) only
    let list = Todo::filter_by_user_id(user.id).exec(&mut db).await?;
    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    // Find the user via the todo's FK
    let user_reload = User::get_by_id(&mut db, &todo.user_id).await?;
    assert_eq!(user.id, user_reload.id);

    let mut created = HashMap::new();
    let mut ids = vec![todo.id];
    created.insert(todo.id, todo);

    for i in 0..5 {
        let title = format!("hello world {i}");
        let todo = if i.is_even() {
            user.todos().create().title(title).exec(&mut db).await?
        } else {
            Todo::create()
                .user(&user)
                .title(title)
                .exec(&mut db)
                .await?
        };
        ids.push(todo.id);
        assert_none!(created.insert(todo.id, todo));
    }

    let list = user.todos().exec(&mut db).await?;
    assert_eq!(6, list.len());

    let loaded: HashMap<_, _> = list.into_iter().map(|t| (t.id, t)).collect();
    assert_eq!(6, loaded.len());
    for (id, expect) in &created {
        assert_eq!(expect.title, loaded[id].title);
    }

    let list = Todo::filter_by_user_id(user.id).exec(&mut db).await?;
    assert_eq!(6, list.len());

    let user2 = User::create().name("User 2").exec(&mut db).await?;
    assert_eq!(0, user2.todos().exec(&mut db).await?.len());

    let u2_todo = user2
        .todos()
        .create()
        .title("user 2 todo")
        .exec(&mut db)
        .await?;

    for todo in user.todos().exec(&mut db).await? {
        assert_ne!(u2_todo.id, todo.id);
    }

    // Delete a TODO by value (lookup by composite PK)
    let todo = Todo::get_by_user_id_and_id(&mut db, &user.id, &ids[0]).await?;
    todo.delete().exec(&mut db).await?;
    assert_err!(Todo::get_by_user_id_and_id(&mut db, &user.id, &ids[0]).await);
    assert_err!(user.todos().get_by_id(&mut db, &ids[0]).await);

    // Delete a TODO by scope
    user.todos()
        .filter_by_id(ids[1])
        .delete()
        .exec(&mut db)
        .await?;
    assert_err!(Todo::get_by_user_id_and_id(&mut db, &user.id, &ids[1]).await);
    assert_err!(user.todos().get_by_id(&mut db, &ids[1]).await);

    // Update a TODO via scope
    user.todos()
        .filter_by_id(ids[2])
        .update()
        .title("batch update 1")
        .exec(&mut db)
        .await?;
    let todo = Todo::get_by_user_id_and_id(&mut db, &user.id, &ids[2]).await?;
    assert_eq!(todo.title, "batch update 1");

    // Updating via the wrong user's scope must NOT touch the todo
    user2
        .todos()
        .filter_by_id(ids[2])
        .update()
        .title("batch update 2")
        .exec(&mut db)
        .await?;
    let todo = Todo::get_by_user_id_and_id(&mut db, &user.id, &ids[2]).await?;
    assert_eq!(todo.title, "batch update 1");

    // Deleting the parent deletes its todos
    let id = user.id;
    user.delete().exec(&mut db).await?;
    assert_err!(User::get_by_id(&mut db, &id).await);
    assert_err!(Todo::get_by_user_id_and_id(&mut db, &id, &ids[2]).await);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_has_many_insert_on_update(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Alice").exec(&mut db).await?;
    assert!(user.todos().exec(&mut db).await?.is_empty());

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

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_scoped_find_by_id(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user1 = User::create().name("User 1").exec(&mut db).await?;
    let user2 = User::create().name("User 2").exec(&mut db).await?;

    let todo = user1
        .todos()
        .create()
        .title("hello world")
        .exec(&mut db)
        .await?;

    let reloaded = user1.todos().get_by_id(&mut db, &todo.id).await?;
    assert_eq!(reloaded.id, todo.id);
    assert_eq!(reloaded.title, todo.title);

    // Other user's scope: must not find
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

    // Delete in the wrong scope is a no-op
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

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_belongs_to_required(test: &mut Test) {
    let mut db = setup(test).await;
    assert_err!(Todo::create().title("orphan").exec(&mut db).await);
}

#[driver_test(id(ID))]
pub async fn composite_delete_when_belongs_to_optional(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    // Composite FK where the FK is *optional* — modeled by leaving the
    // partition-key column as `Option<ID>`. (NB: keys must still be
    // populated when inserting, but the nullable FK exercises the
    // belongs_to-optional codepath after the parent is deleted.)
    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    let user = User::create().exec(&mut db).await?;
    let mut ids = vec![];

    for _ in 0..3 {
        let todo = user.todos().create().exec(&mut db).await?;
        ids.push(todo.id);
    }

    user.delete().exec(&mut db).await?;

    // Todos still exist; user_id is None
    for id in ids {
        let todo = Todo::get_by_id(&mut db, id).await?;
        assert_none!(todo.user_id);
    }
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_associate_new_user_with_todo_on_update_via_creation(
    test: &mut Test,
) -> Result<()> {
    let mut db = setup(test).await;

    let u1 = User::create()
        .name("User 1")
        .todo(Todo::create().title("hello world"))
        .exec(&mut db)
        .await?;

    let todos: Vec<_> = u1.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    let mut todo = todos.into_iter().next().unwrap();

    todo.update()
        .user(User::create().name("User 2"))
        .exec(&mut db)
        .await?;
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_associate_new_user_with_todo_on_update_query_via_creation(
    test: &mut Test,
) -> Result<()> {
    let mut db = setup(test).await;

    let u1 = User::create()
        .name("User 1")
        .todo(Todo::create().title("a todo"))
        .exec(&mut db)
        .await?;

    let todos: Vec<_> = u1.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    let todo = todos.into_iter().next().unwrap();

    Todo::filter_by_user_id_and_id(todo.user_id, todo.id)
        .update()
        .user(User::create().name("User 2"))
        .exec(&mut db)
        .await?;
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_assign_todo_that_already_has_user_on_create(test: &mut Test) -> Result<()> {
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

    let todo_reload = Todo::get_by_user_id_and_id(&mut db, &u2.id, &todo.id).await?;
    assert_eq!(u2.id, todo_reload.user_id);

    let todos: Vec<_> = u1.todos().exec(&mut db).await?;
    assert_eq!(0, todos.len());

    let todos: Vec<_> = u2.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    assert_eq!(todo.id, todos[0].id);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_assign_todo_that_already_has_user_on_update(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let todo = Todo::create()
        .title("a todo")
        .user(User::create().name("User 1"))
        .exec(&mut db)
        .await?;

    let u1 = todo.user().exec(&mut db).await?;

    let mut u2 = User::create().name("User 2").exec(&mut db).await?;

    u2.update()
        .todos(toasty::stmt::insert(&todo))
        .exec(&mut db)
        .await?;

    let todo_reload = Todo::get_by_user_id_and_id(&mut db, &u2.id, &todo.id).await?;
    assert_eq!(u2.id, todo_reload.user_id);

    let todos: Vec<_> = u1.todos().exec(&mut db).await?;
    assert_eq!(0, todos.len());

    let todos: Vec<_> = u2.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    assert_eq!(todo.id, todos[0].id);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_assign_existing_user_to_todo(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut todo = Todo::create()
        .title("hello")
        .user(User::create().name("User 1"))
        .exec(&mut db)
        .await?;

    let u1 = todo.user().exec(&mut db).await?;
    let u2 = User::create().name("User 2").exec(&mut db).await?;

    todo.update().user(&u2).exec(&mut db).await?;

    let todo_reload = Todo::get_by_user_id_and_id(&mut db, &u2.id, &todo.id).await?;
    assert_eq!(u2.id, todo_reload.user_id);

    let todos: Vec<_> = u1.todos().exec(&mut db).await?;
    assert_eq!(0, todos.len());

    let todos: Vec<_> = u2.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    assert_eq!(todo.id, todos[0].id);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_assign_todo_to_user_on_update_query(test: &mut Test) -> Result<()> {
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

// =====================================================================
// Tier B — batch / link / unlink. Mirrors `relation_has_many_batch_create.rs`
// and `relation_has_many_link_unlink.rs`.
// =====================================================================

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_user_batch_create_todos_one_level(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = User::create()
        .name("Ann Chovey")
        .todo(Todo::create().title("Make pizza"))
        .exec(&mut db)
        .await?;

    assert_eq!(user.name, "Ann Chovey");

    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    assert_eq!("Make pizza", todos[0].title);

    let todo = Todo::get_by_user_id_and_id(&mut db, &user.id, &todos[0].id).await?;
    assert_eq!("Make pizza", todo.title);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_user_batch_create_two_todos_simple(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = User::create()
        .name("Ann Chovey")
        .todo(Todo::create().title("Make pizza"))
        .todo(Todo::create().title("Sleep"))
        .exec(&mut db)
        .await?;

    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(2, todos.len());

    let mut titles: Vec<_> = todos.iter().map(|t| &t.title[..]).collect();
    titles.sort();
    assert_eq!(titles, vec!["Make pizza", "Sleep"]);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn composite_user_batch_create_todos_with_optional_field(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,

        // Optional field exercises the RETURNING/constantize path that
        // regressed in #user_batch_create_todos_with_optional_field.
        #[allow(dead_code)]
        moto: Option<String>,
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = user_id, local = id)]
    struct Todo {
        #[auto]
        id: uuid::Uuid,

        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    let user = User::create()
        .name("Ann Chovey")
        .todo(Todo::create().title("Make pizza"))
        .todo(Todo::create().title("Sleep"))
        .exec(&mut db)
        .await?;

    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(2, todos.len());

    let mut titles: Vec<_> = todos.iter().map(|t| &t.title[..]).collect();
    titles.sort();
    assert_eq!(titles, vec!["Make pizza", "Sleep"]);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn composite_remove_add_single_relation_option_belongs_to(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    let user = User::create()
        .todo(Todo::create())
        .todo(Todo::create())
        .exec(&mut db)
        .await?;

    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(2, todos.len());

    user.todos().remove(&mut db, &todos[0]).await?;

    let todos_reloaded: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(1, todos_reloaded.len());
    assert_eq!(todos[1].id, todos_reloaded[0].id);

    assert_err!(user.todos().get_by_id(&mut db, &todos[0].id).await);

    let todo = Todo::get_by_id(&mut db, todos[0].id).await?;
    assert_none!(todo.user_id);

    user.todos().insert(&mut db, &todos[0]).await?;

    let todos_reloaded: Vec<_> = user.todos().exec(&mut db).await?;
    assert!(todos_reloaded.iter().any(|t| t.id == todos[0].id));
    assert_ok!(user.todos().get_by_id(&mut db, todos[0].id).await);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_add_remove_single_relation_required_belongs_to(
    test: &mut Test,
) -> Result<()> {
    let mut db = setup(test).await;

    let user = User::create().name("User 1").exec(&mut db).await?;

    let t1 = user.todos().create().title("todo 1").exec(&mut db).await?;
    let t2 = user.todos().create().title("todo 2").exec(&mut db).await?;
    let t3 = user.todos().create().title("todo 3").exec(&mut db).await?;

    let ids = vec![t1.id, t2.id, t3.id];

    let todos_reloaded: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(todos_reloaded.len(), 3);

    for id in ids {
        assert!(todos_reloaded.iter().any(|t| t.id == id));
    }

    // Unlinking a required belongs_to is a delete
    user.todos().remove(&mut db, &todos_reloaded[0]).await?;

    assert_err!(Todo::get_by_user_id_and_id(&mut db, &user.id, &todos_reloaded[0].id).await);

    let todos_reloaded: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(todos_reloaded.len(), 2);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_reassign_relation_required_belongs_to(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let u1 = User::create().name("User 1").exec(&mut db).await?;
    let u2 = User::create().name("User 2").exec(&mut db).await?;

    let t1 = u1.todos().create().title("a todo").exec(&mut db).await?;

    u2.todos().insert(&mut db, &t1).await?;

    assert!(u1.todos().exec(&mut db).await?.is_empty());

    let todos = u2.todos().exec(&mut db).await?;
    assert_eq!(1, todos.len());
    assert_eq!(t1.id, todos[0].id);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn composite_add_remove_multiple_relation_option_belongs_to(
    test: &mut Test,
) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    let user = User::create().exec(&mut db).await?;

    let t1 = Todo::create().exec(&mut db).await?;
    let t2 = Todo::create().exec(&mut db).await?;
    let t3 = Todo::create().exec(&mut db).await?;

    let ids = vec![t1.id, t2.id, t3.id];

    user.todos().insert(&mut db, &t1).await?;
    user.todos().insert(&mut db, &t2).await?;
    user.todos().insert(&mut db, &t3).await?;

    let todos_reloaded: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(todos_reloaded.len(), 3);

    for id in ids {
        assert!(todos_reloaded.iter().any(|t| t.id == id));
    }
    Ok(())
}

// =====================================================================
// Tier C — preload (.include) over a composite FK. Mirrors
// `relation_preload.rs`.
// =====================================================================

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_basic_has_many_and_belongs_to_preload(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = User::create()
        .name("Alice")
        .todo(Todo::create().title("todo 1"))
        .todo(Todo::create().title("todo 2"))
        .todo(Todo::create().title("todo 3"))
        .exec(&mut db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().todos())
        .get(&mut db)
        .await?;

    assert_eq!(3, user.todos.get().len());

    let id = user.todos.get()[0].id;
    let user_id = user.todos.get()[0].user_id;

    let todo = Todo::filter_by_user_id_and_id(user_id, id)
        .include(Todo::fields().user())
        .get(&mut db)
        .await?;

    assert_eq!(user.id, todo.user.get().id);
    assert_eq!(user.id, todo.user_id);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_preload_on_empty_query(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    User::create().name("Alice").exec(&mut db).await?;

    let users: Vec<_> = User::filter(User::fields().name().eq("Nope"))
        .include(User::fields().todos())
        .exec(&mut db)
        .await?;
    assert!(users.is_empty());
    Ok(())
}

#[driver_test(id(ID))]
pub async fn composite_preload_has_many_with_optional_belongs_to(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

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
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    let user = User::create()
        .name("Alice")
        .todo(Todo::create().title("alpha"))
        .todo(Todo::create().title("beta"))
        .exec(&mut db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().todos())
        .get(&mut db)
        .await?;

    assert_eq!(2, user.todos.get().len());
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::composite_has_many_belongs_to))]
pub async fn composite_nested_has_many_then_belongs_to_required(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = User::create()
        .name("Alice")
        .todo(Todo::create().title("alpha"))
        .todo(Todo::create().title("beta"))
        .exec(&mut db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().todos().user())
        .get(&mut db)
        .await?;

    let todos = user.todos.get();
    assert_eq!(2, todos.len());
    for todo in todos {
        assert_eq!(user.id, todo.user.get().id);
        assert_eq!("Alice", todo.user.get().name);
    }
    Ok(())
}

// =====================================================================
// Tier D — has_one / belongs_to topologies with composite FK on the child.
// =====================================================================

#[driver_test(id(ID))]
pub async fn composite_crud_has_one_required(test: &mut Test) -> Result<()> {
    // User has a single auto-PK and a `has_one` Profile.
    // Profile's PK is composite (`partition = user_id, local = id`), making the
    // FK part of the row's key.
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = user_id, local = id)]
    struct Profile {
        #[auto]
        id: uuid::Uuid,

        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        bio: String,
    }

    let mut db = test.setup_db(models!(User, Profile)).await;

    let user = User::create()
        .profile(Profile::create().bio("an apple a day"))
        .exec(&mut db)
        .await?;

    let profile = user.profile().exec(&mut db).await?.unwrap();
    assert_eq!(profile.bio, "an apple a day");

    assert_eq!(user.id, profile.user().exec(&mut db).await?.id);

    // Deleting the user also deletes the profile.
    user.delete().exec(&mut db).await?;
    assert_err!(Profile::get_by_user_id_and_id(&mut db, &profile.user_id, &profile.id).await);
    Ok(())
}

// =====================================================================
// Tier E — filters and projections through associations.
// =====================================================================

#[driver_test(id(ID), requires(sql))]
pub async fn composite_filter_by_belongs_to_field(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = user_id, local = id)]
    struct Profile {
        #[auto]
        id: uuid::Uuid,

        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        bio: String,
    }

    let mut db = test.setup_db(models!(User, Profile)).await;

    let alice = User::create().name("alice").exec(&mut db).await?;
    let bob = User::create().name("bob").exec(&mut db).await?;

    Profile::create()
        .user(&alice)
        .bio("alice's bio")
        .exec(&mut db)
        .await?;
    Profile::create()
        .user(&bob)
        .bio("bob's bio")
        .exec(&mut db)
        .await?;

    let profiles: Vec<Profile> = Profile::filter(Profile::fields().user().name().eq("alice"))
        .exec(&mut db)
        .await?;
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].bio, "alice's bio");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(scan),
    scenario(crate::scenarios::composite_has_many_belongs_to)
)]
pub async fn composite_filter_parent_by_child_field(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let alice = User::create().name("Alice").exec(&mut db).await?;
    let bob = User::create().name("Bob").exec(&mut db).await?;
    let carol = User::create().name("Carol").exec(&mut db).await?;

    alice.todos().create().title("urgent").exec(&mut db).await?;
    bob.todos().create().title("later").exec(&mut db).await?;
    carol.todos().create().title("urgent").exec(&mut db).await?;
    carol.todos().create().title("later").exec(&mut db).await?;

    let users: Vec<_> = User::filter(
        User::fields()
            .todos()
            .any(Todo::fields().title().eq("urgent")),
    )
    .exec(&mut db)
    .await?;

    assert_eq_unordered!(users.iter().map(|u| &u.name[..]), ["Alice", "Carol"]);
    Ok(())
}

#[driver_test(id(ID), requires(scan))]
pub async fn composite_select_belongs_to_basic(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = user_id, local = id)]
    struct Post {
        #[auto]
        id: uuid::Uuid,
        title: String,

        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        author: toasty::BelongsTo<User>,
    }

    let mut db = test.setup_db(models!(User, Post)).await;

    let alice = User::create().name("Alice").exec(&mut db).await?;
    Post::create()
        .title("Hello")
        .author(&alice)
        .exec(&mut db)
        .await?;

    let users: Vec<User> = Post::all()
        .select(Post::fields().author())
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");
    Ok(())
}
