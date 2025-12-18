use tests::{assert_eq_unordered, models, tests, DbTest};
use toasty::stmt::Id;

async fn user_batch_create_todos_one_level_basic_fk(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: Id<Self>,

        #[index]
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a user with some todos
    let user = User::create()
        .name("Ann Chovey")
        .todo(Todo::create().title("Make pizza"))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(user.name, "Ann Chovey");

    // There are associated TODOs
    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!("Make pizza", todos[0].title);

    // Find the todo by ID
    let todo = Todo::get_by_id(&db, &todos[0].id).await.unwrap();
    assert_eq!("Make pizza", todo.title);
}

async fn user_batch_create_todos_two_levels_basic_fk(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: Id<Self>,

        #[index]
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[index]
        category_id: Id<Category>,

        #[belongs_to(key = category_id, references = id)]
        category: toasty::BelongsTo<Category>,

        title: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Category {
        #[key]
        #[auto]
        id: Id<Self>,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    let db = test.setup_db(models!(User, Todo, Category)).await;

    // Create a user with some todos
    let user = User::create()
        .name("Ann Chovey")
        .todo(
            Todo::create()
                .title("Make pizza")
                .category(Category::create().name("Eating")),
        )
        .exec(&db)
        .await
        .unwrap();
    assert_eq!(user.name, "Ann Chovey");

    // There are associated TODOs
    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!("Make pizza", todos[0].title);

    // Find the todo by ID
    let todo = Todo::get_by_id(&db, &todos[0].id).await.unwrap();
    assert_eq!("Make pizza", todo.title);

    // Find the category by ID
    let category = Category::get_by_id(&db, &todo.category_id).await.unwrap();
    assert_eq!(category.name, "Eating");

    // Create more than one todo per user
    let user = User::create()
        .name("John Doe")
        .todo(
            Todo::create()
                .title("do something")
                .category(Category::create().name("things")),
        )
        .todo(
            Todo::create()
                .title("do something else")
                .category(Category::create().name("other things")),
        )
        .exec(&db)
        .await
        .unwrap();

    // There are associated TODOs
    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq_unordered!(
        todos.iter().map(|todo| &todo.title[..]),
        ["do something", "do something else"]
    );

    let mut categories = vec![];

    for todo in &todos {
        categories.push(todo.category().get(&db).await.unwrap());
    }

    assert_eq_unordered!(
        categories.iter().map(|category| &category.name[..]),
        ["things", "other things"]
    );

    let todos: Vec<_> = category.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
}

async fn user_batch_create_todos_set_category_by_value(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: Id<Self>,

        #[index]
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[index]
        category_id: Id<Category>,

        #[belongs_to(key = category_id, references = id)]
        category: toasty::BelongsTo<Category>,

        title: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Category {
        #[key]
        #[auto]
        id: Id<Self>,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    let db = test.setup_db(models!(User, Todo, Category)).await;

    let category = Category::create().name("Eating").exec(&db).await.unwrap();
    assert_eq!(category.name, "Eating");

    let user = User::create()
        .name("John Doe")
        .todo(Todo::create().title("Pizza").category(&category))
        .todo(Todo::create().title("Hamburger").category(&category))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(user.name, "John Doe");

    // There are associated TODOs
    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq_unordered!(
        todos.iter().map(|todo| &todo.title[..]),
        ["Pizza", "Hamburger"]
    );

    for todo in &todos {
        assert_eq!(todo.category_id, category.id);
    }

    let todos: Vec<_> = category.todos().collect(&db).await.unwrap();
    assert_eq_unordered!(
        todos.iter().map(|todo| &todo.title[..]),
        ["Pizza", "Hamburger"]
    );
}

async fn user_batch_create_todos_set_category_by_query(_test: &mut DbTest) {}

/// Regression test for batch creation with optional fields
///
/// This test reproduces a panic that occurs when:
/// 1. A parent model has an optional field (e.g., `moto: Option<String>`)
/// 2. The parent has a has_many relationship with auto-increment IDs
/// 3. You batch-create multiple associated records in a single operation
///
/// The panic occurs at crates/toasty/src/engine/lower/insert.rs:192 with:
/// "not yet implemented: expr=ExprStmt { ... }"
///
/// The issue is in the RETURNING clause constantization code path where
/// batch inserts with auto-increment fields encounter an Expr::Stmt (nested insert)
/// that is not yet handled.
async fn user_batch_create_todos_with_optional_field(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,

        // This optional field triggers the unimplemented code path!
        // Without it, the batch create works fine.
        #[allow(dead_code)]
        moto: Option<String>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: Id<Self>,

        #[index]
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // This operation currently panics due to unimplemented code path
    let user = User::create()
        .name("Ann Chovey")
        .todo(Todo::create().title("Make pizza"))
        .todo(Todo::create().title("Sleep"))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(user.name, "Ann Chovey");

    // Verify both todos were created
    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(2, todos.len());

    let mut titles: Vec<_> = todos.iter().map(|t| &t.title[..]).collect();
    titles.sort();
    assert_eq!(titles, vec!["Make pizza", "Sleep"]);
}

async fn user_batch_create_two_todos_simple(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        name: String,

        #[unique]
        #[allow(dead_code)]
        email: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: Id<Self>,

        #[index]
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a user with two todos in a single operation
    let user = User::create()
        .name("Ann Chovey")
        .email("ann.chovey@example.com")
        .todo(Todo::create().title("Make pizza"))
        .todo(Todo::create().title("Sleep"))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(user.name, "Ann Chovey");

    // There should be 2 associated TODOs
    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(2, todos.len());

    // Verify the titles
    let mut titles: Vec<_> = todos.iter().map(|t| &t.title[..]).collect();
    titles.sort();
    assert_eq!(titles, vec!["Make pizza", "Sleep"]);
}

tests!(
    user_batch_create_todos_one_level_basic_fk,
    user_batch_create_todos_two_levels_basic_fk,
    user_batch_create_todos_set_category_by_value,
    #[ignore]
    user_batch_create_todos_set_category_by_query,
    user_batch_create_two_todos_simple,
    user_batch_create_todos_with_optional_field,
);
