//! Test batching association-scoped statements (create, query, update, delete)
//! through `toasty::batch()`.

use crate::prelude::*;

/// Batch two association-scoped creates on the same relation.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_two_scoped_creates_same_relation(t: &mut Test) -> Result<()> {
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
        id: ID,
        #[index]
        user_id: ID,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
    }

    let mut db = t.setup_db(models!(User, Todo)).await;
    let user = User::create().exec(&mut db).await?;

    let (t1, t2): (Todo, Todo) = toasty::batch((
        user.todos().create().title("first"),
        user.todos().create().title("second"),
    ))
    .exec(&mut db)
    .await?;

    assert_eq!(t1.title, "first");
    assert_eq!(t2.title, "second");
    assert_eq!(t1.user_id, user.id);
    assert_eq!(t2.user_id, user.id);

    let all: Vec<Todo> = user.todos().collect(&mut db).await?;
    assert_eq!(all.len(), 2);

    Ok(())
}

/// Batch two association-scoped queries on the same relation.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_two_scoped_queries_same_relation(t: &mut Test) -> Result<()> {
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
        id: ID,
        #[index]
        user_id: ID,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        #[index]
        title: String,
    }

    let mut db = t.setup_db(models!(User, Todo)).await;

    let u1 = User::create().exec(&mut db).await?;
    let u2 = User::create().exec(&mut db).await?;
    u1.todos().create().title("u1 todo").exec(&mut db).await?;
    u2.todos().create().title("u2 todo").exec(&mut db).await?;

    let (u1_todos, u2_todos): (Vec<Todo>, Vec<Todo>) = toasty::batch((u1.todos(), u2.todos()))
        .exec(&mut db)
        .await?;

    assert_struct!(u1_todos, [_ { title: "u1 todo" }]);
    assert_struct!(u2_todos, [_ { title: "u2 todo" }]);

    Ok(())
}

/// Batch association-scoped update and delete on the same relation.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_scoped_update_and_delete_same_relation(t: &mut Test) -> Result<()> {
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
        id: ID,
        #[index]
        user_id: ID,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
    }

    let mut db = t.setup_db(models!(User, Todo)).await;
    let user = User::create().exec(&mut db).await?;
    let todo_keep = user.todos().create().title("keep").exec(&mut db).await?;
    let todo_drop = user.todos().create().title("drop").exec(&mut db).await?;

    let ((), ()): ((), ()) = toasty::batch((
        user.todos()
            .filter_by_id(todo_keep.id)
            .update()
            .title("kept"),
        user.todos().filter_by_id(todo_drop.id).delete(),
    ))
    .exec(&mut db)
    .await?;

    let remaining: Vec<Todo> = user.todos().collect(&mut db).await?;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].title, "kept");

    Ok(())
}

/// Batch all four CRUD operations through association scope.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_scoped_all_four_crud(t: &mut Test) -> Result<()> {
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
        id: ID,
        #[index]
        user_id: ID,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
    }

    let mut db = t.setup_db(models!(User, Todo)).await;
    let user = User::create().exec(&mut db).await?;
    let existing = user
        .todos()
        .create()
        .title("existing")
        .exec(&mut db)
        .await?;
    let doomed = user.todos().create().title("doomed").exec(&mut db).await?;

    let (queried, created, (), ()): (Vec<Todo>, Todo, (), ()) = toasty::batch((
        user.todos(),
        user.todos().create().title("new"),
        user.todos()
            .filter_by_id(existing.id)
            .update()
            .title("updated"),
        user.todos().filter_by_id(doomed.id).delete(),
    ))
    .exec(&mut db)
    .await?;

    // Query ran before the update/delete in this batch, so sees original state
    assert_eq!(queried.len(), 2);
    assert_eq!(created.title, "new");

    // Verify final state
    let final_todos: Vec<Todo> = user.todos().collect(&mut db).await?;
    assert_eq!(final_todos.len(), 2); // "updated" + "new", "doomed" deleted

    let titles: Vec<&str> = final_todos.iter().map(|t| t.title.as_str()).collect();
    assert!(titles.contains(&"updated"));
    assert!(titles.contains(&"new"));

    Ok(())
}

/// Batch association-scoped statements mixed with root-level statements.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_scoped_with_root_statements(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[index]
        name: String,
        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,
        #[index]
        user_id: ID,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
    }

    let mut db = t.setup_db(models!(User, Todo)).await;
    let user = User::create().name("Alice").exec(&mut db).await?;

    // Mix: root-level query + scoped create + root-level create
    let (users, todo, new_user): (Vec<User>, Todo, User) = toasty::batch((
        User::filter_by_name("Alice"),
        user.todos().create().title("from batch"),
        User::create().name("Bob"),
    ))
    .exec(&mut db)
    .await?;

    assert_struct!(users, [_ { name: "Alice" }]);
    assert_eq!(todo.title, "from batch");
    assert_eq!(todo.user_id, user.id);
    assert_eq!(new_user.name, "Bob");

    Ok(())
}

/// Batch association statements across different relations of the same parent.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_scoped_across_relations(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[has_many]
        todos: toasty::HasMany<Todo>,
        #[has_many]
        posts: toasty::HasMany<Post>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,
        #[index]
        user_id: ID,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,
        #[index]
        user_id: ID,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        body: String,
    }

    let mut db = t.setup_db(models!(User, Todo, Post)).await;
    let user = User::create().exec(&mut db).await?;

    // Create across two different relations in one batch
    let (todo, post): (Todo, Post) = toasty::batch((
        user.todos().create().title("my todo"),
        user.posts().create().body("my post"),
    ))
    .exec(&mut db)
    .await?;

    assert_eq!(todo.title, "my todo");
    assert_eq!(todo.user_id, user.id);
    assert_eq!(post.body, "my post");
    assert_eq!(post.user_id, user.id);

    Ok(())
}

/// Batch queries across different relations of the same parent.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_query_across_relations(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[has_many]
        todos: toasty::HasMany<Todo>,
        #[has_many]
        posts: toasty::HasMany<Post>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,
        #[index]
        user_id: ID,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,
        #[index]
        user_id: ID,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        body: String,
    }

    let mut db = t.setup_db(models!(User, Todo, Post)).await;
    let user = User::create().exec(&mut db).await?;
    user.todos().create().title("t1").exec(&mut db).await?;
    user.todos().create().title("t2").exec(&mut db).await?;
    user.posts().create().body("p1").exec(&mut db).await?;

    let (todos, posts): (Vec<Todo>, Vec<Post>) = toasty::batch((user.todos(), user.posts()))
        .exec(&mut db)
        .await?;

    assert_eq!(todos.len(), 2);
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].body, "p1");

    Ok(())
}

/// Batch scoped operations from different parents.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_scoped_different_parents(t: &mut Test) -> Result<()> {
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
        id: ID,
        #[index]
        user_id: ID,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
    }

    let mut db = t.setup_db(models!(User, Todo)).await;
    let alice = User::create().exec(&mut db).await?;
    let bob = User::create().exec(&mut db).await?;

    // Create todos for different users in one batch
    let (alice_todo, bob_todo): (Todo, Todo) = toasty::batch((
        alice.todos().create().title("alice task"),
        bob.todos().create().title("bob task"),
    ))
    .exec(&mut db)
    .await?;

    assert_eq!(alice_todo.user_id, alice.id);
    assert_eq!(bob_todo.user_id, bob.id);

    // Query both scopes in one batch
    let (alice_todos, bob_todos): (Vec<Todo>, Vec<Todo>) =
        toasty::batch((alice.todos(), bob.todos()))
            .exec(&mut db)
            .await?;

    assert_struct!(alice_todos, [_ { title: "alice task" }]);
    assert_struct!(bob_todos, [_ { title: "bob task" }]);

    Ok(())
}

/// Batch a scoped delete together with a root-level update.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_scoped_delete_with_root_update(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[index]
        name: String,
        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,
        #[index]
        user_id: ID,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
    }

    let mut db = t.setup_db(models!(User, Todo)).await;
    let user = User::create().name("Alice").exec(&mut db).await?;
    let todo = user.todos().create().title("done").exec(&mut db).await?;

    let ((), ()): ((), ()) = toasty::batch((
        user.todos().filter_by_id(todo.id).delete(),
        User::filter_by_name("Alice").update().name("Alice2"),
    ))
    .exec(&mut db)
    .await?;

    // Todo deleted
    let remaining: Vec<Todo> = user.todos().collect(&mut db).await?;
    assert!(remaining.is_empty());

    // User updated
    let updated: Vec<User> = User::filter_by_name("Alice2").collect(&mut db).await?;
    assert_eq!(updated.len(), 1);

    Ok(())
}
