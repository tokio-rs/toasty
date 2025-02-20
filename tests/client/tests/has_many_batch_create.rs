use tests_client::*;

async fn user_batch_create_todos_one_level_basic_fk(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            name: String,

            todos: [Todo],
        }

        model Todo {
            #[key]
            #[auto]
            id: Id,

            #[index]
            user_id: Id<User>,

            #[relation(key = user_id, references = id)]
            user: User,

            title: String,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user with some todos
    let user = db::User::create()
        .name("Ann Chovey")
        .todo(db::Todo::create().title("Make pizza"))
        .exec(&db)
        .await
        .unwrap();

    // There are associated TODOs
    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!("Make pizza", todos[0].title);

    // Find the todo by ID
    let todo = db::Todo::get_by_id(&db, &todos[0].id).await.unwrap();
    assert_eq!("Make pizza", todo.title);
}

async fn user_batch_create_todos_two_levels_basic_fk(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            name: String,

            todos: [Todo],
        }

        model Todo {
            #[key]
            #[auto]
            id: Id,

            #[index]
            user_id: Id<User>,

            #[relation(key = user_id, references = id)]
            user: User,

            #[index]
            category_id: Id<Category>,

            #[relation(key = category_id, references = id)]
            category: Category,

            title: String,
        }

        model Category {
            #[key]
            #[auto]
            id: Id,

            name: String,

            todos: [Todo],
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user with some todos
    let user = db::User::create()
        .name("Ann Chovey")
        .todo(
            db::Todo::create()
                .title("Make pizza")
                .category(db::Category::create().name("Eating")),
        )
        .exec(&db)
        .await
        .unwrap();

    // There are associated TODOs
    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!("Make pizza", todos[0].title);

    // Find the todo by ID
    let todo = db::Todo::get_by_id(&db, &todos[0].id).await.unwrap();
    assert_eq!("Make pizza", todo.title);

    // Find the category by ID
    let category = db::Category::get_by_id(&db, &todo.category_id)
        .await
        .unwrap();
    assert_eq!(category.name, "Eating");

    // Create more than one todo per user
    let user = db::User::create()
        .name("John Doe")
        .todo(
            db::Todo::create()
                .title("do something")
                .category(db::Category::create().name("things")),
        )
        .todo(
            db::Todo::create()
                .title("do something else")
                .category(db::Category::create().name("other things")),
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
}

async fn user_batch_create_todos_set_category_by_value(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            name: String,

            todos: [Todo],
        }

        model Todo {
            #[key]
            #[auto]
            id: Id,

            #[index]
            user_id: Id<User>,

            #[relation(key = user_id, references = id)]
            user: User,

            #[index]
            category_id: Id<Category>,

            #[relation(key = category_id, references = id)]
            category: Category,

            title: String,
        }

        model Category {
            #[key]
            #[auto]
            id: Id,

            name: String,

            todos: [Todo],
        }"
    );

    let db = s.setup(db::load_schema()).await;

    let category = db::Category::create()
        .name("Eating")
        .exec(&db)
        .await
        .unwrap();

    let user = db::User::create()
        .name("John Doe")
        .todo(db::Todo::create().title("Pizza").category(&category))
        .todo(db::Todo::create().title("Hamburger").category(&category))
        .exec(&db)
        .await
        .unwrap();

    // There are associated TODOs
    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq_unordered!(
        todos.iter().map(|todo| &todo.title[..]),
        ["Pizza", "Hamburger"]
    );

    for todo in &todos {
        assert_eq!(todo.category_id, category.id);
    }
}

async fn user_batch_create_todos_set_category_by_query(_s: impl Setup) {}

tests!(
    user_batch_create_todos_one_level_basic_fk,
    user_batch_create_todos_two_levels_basic_fk,
    user_batch_create_todos_set_category_by_value,
    #[ignore]
    user_batch_create_todos_set_category_by_query,
);
