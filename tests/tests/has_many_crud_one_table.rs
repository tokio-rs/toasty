use tests::*;
use toasty::stmt::Id;

use std::collections::HashMap;
use std_util::prelude::*;

async fn crud_user_todos(s: impl Setup) {
    #[derive(Debug, toasty::Model)]
    #[table = "user_and_todos"]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[table = "user_and_todos"]
    #[key(partition = user_id, local = id)]
    struct Todo {
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[auto]
        id: Id<Self>,

        title: String,
    }

    let db = s.setup(models!(User, Todo)).await;

    // Create a user
    let user = User::create().exec(&db).await.unwrap();

    // No TODOs
    assert_empty!(user
        .todos()
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap());

    // Create a Todo associated with the user
    let todo = user
        .todos()
        .create()
        .title("hello world")
        .exec(&db)
        .await
        .unwrap();

    // Find the todo by user & ID
    let list = Todo::filter_by_user_id_and_id(&user.id, &todo.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    // Find the TODO by user ID
    let list = Todo::filter_by_user_id(&user.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    let mut created = HashMap::new();
    let mut ids = vec![todo.id.clone()];
    created.insert(todo.id.clone(), todo);

    // Create a few more TODOs
    for i in 0..5 {
        let title = format!("hello world {i}");

        let todo = if i.is_even() {
            // Create via user
            user.todos().create().title(title).exec(&db).await.unwrap()
        } else {
            // Create via todo builder
            Todo::create()
                .user(&user)
                .title(title)
                .exec(&db)
                .await
                .unwrap()
        };

        ids.push(todo.id.clone());
        assert_none!(created.insert(todo.id.clone(), todo));
    }

    // Load all TODOs
    let list = user
        .todos()
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(6, list.len());

    let loaded: HashMap<_, _> = list
        .into_iter()
        .map(|todo| (todo.id.clone(), todo))
        .collect();

    for (id, expect) in &created {
        assert_eq!(expect.title, loaded[id].title);
    }

    // Create a second user
    let user2 = User::create().exec(&db).await.unwrap();

    // No TODOs associated with `user2`
    assert_empty!(user2
        .todos()
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap());

    // Create a TODO for user2
    let u2_todo = user2
        .todos()
        .create()
        .title("user 2 todo")
        .exec(&db)
        .await
        .unwrap();

    let mut u1_todos = user.todos().all(&db).await.unwrap();

    while let Some(todo) = u1_todos.next().await {
        let todo = todo.unwrap();
        assert_ne!(u2_todo.id, todo.id);
    }

    // Delete a TODO by value
    let todo = Todo::get_by_user_id_and_id(&db, &user.id, &ids[0])
        .await
        .unwrap();
    todo.delete(&db).await.unwrap();

    // Can no longer get the todo via id
    assert_err!(Todo::get_by_user_id_and_id(&db, &user.id, &ids[0]).await);

    // Can no longer get the todo scoped
    assert_err!(user.todos().get_by_id(&db, &ids[0]).await);

    // Delete a TODO by scope
    user.todos()
        .filter_by_id(&ids[1])
        .delete(&db)
        .await
        .unwrap();

    // Can no longer get the todo via id
    assert_err!(Todo::get_by_user_id_and_id(&db, &user.id, &ids[1]).await);

    // Can no longer get the todo scoped
    assert_err!(user.todos().get_by_id(&db, &ids[1]).await);

    // Successfuly a todo by scope
    user.todos()
        .filter_by_id(&ids[2])
        .update()
        .title("batch update 1")
        .exec(&db)
        .await
        .unwrap();
    let todo = Todo::get_by_user_id_and_id(&db, &user.id, &ids[2])
        .await
        .unwrap();
    assert_eq!(todo.title, "batch update 1");

    // Now fail to update it by scoping by other user
    user2
        .todos()
        .filter_by_id(&ids[2])
        .update()
        .title("batch update 2")
        .exec(&db)
        .await
        .unwrap();
    let todo = Todo::get_by_user_id_and_id(&db, &user.id, &ids[2])
        .await
        .unwrap();
    assert_eq!(todo.title, "batch update 1");
}

async fn scoped_find_by_id(s: impl Setup) {
    #[derive(Debug, toasty::Model)]
    #[table = "user_and_todos"]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[table = "user_and_todos"]
    #[key(partition = user_id, local = id)]
    struct Todo {
        #[auto]
        id: Id<Self>,

        #[index]
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let db = s.setup(models!(User, Todo)).await;

    // Create a couple of users
    let user1 = User::create().exec(&db).await.unwrap();
    let user2 = User::create().exec(&db).await.unwrap();

    // Create a todo
    let todo = user1
        .todos()
        .create()
        .title("hello world")
        .exec(&db)
        .await
        .unwrap();

    // Find it scoped by user1
    let reloaded = user1.todos().get_by_id(&db, &todo.id).await.unwrap();
    assert_eq!(reloaded.id, todo.id);
    assert_eq!(reloaded.title, todo.title);

    // Trying to find the same todo scoped by user2 is missing
    assert_none!(user2
        .todos()
        .filter_by_id(&todo.id)
        .first(&db)
        .await
        .unwrap());

    let reloaded = User::filter_by_id(&user1.id)
        .todos()
        .get_by_id(&db, &todo.id)
        .await
        .unwrap();

    assert_eq!(reloaded.id, todo.id);
    assert_eq!(reloaded.title, todo.title);

    // Deleting the TODO from the user 2 scope fails
    user2
        .todos()
        .filter_by_id(&todo.id)
        .delete(&db)
        .await
        .unwrap();
    let reloaded = user1.todos().get_by_id(&db, &todo.id).await.unwrap();
    assert_eq!(reloaded.id, todo.id);
}

// The has_many association uses the target's primary key as the association's
// foreign key. In this case, the relation's query should not be duplicated.
async fn has_many_on_target_pk(_s: impl Setup) {}

// The target model has an explicit index on (FK, PK). In this case, the query
// generated by the (FK, PK) pair should not be duplicated by the relation.
async fn has_many_when_target_indexes_fk_and_pk(_s: impl Setup) {}

// When the FK is composite, things should still work
async fn has_many_when_fk_is_composite(_s: impl Setup) {}

// When the PK is composite, things should still work
async fn has_many_when_pk_is_composite(_s: impl Setup) {}

// When both the FK and PK are composite, things should still work
async fn has_many_when_fk_and_pk_are_composite(_s: impl Setup) {}

tests!(
    crud_user_todos,
    scoped_find_by_id,
    has_many_on_target_pk,
    has_many_when_target_indexes_fk_and_pk,
    has_many_when_fk_is_composite,
    has_many_when_pk_is_composite,
    has_many_when_fk_and_pk_are_composite,
);
