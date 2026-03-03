use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn create_macro_simple(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    let mut db = test.setup_db(models!(User)).await;

    // Create using the macro — translates to: User::create().name("Carl")
    let user = toasty::create!(User, { name: "Carl" })
        .exec(&mut db)
        .await?;

    assert_eq!(user.name, "Carl");

    // Verify it persisted
    let reloaded = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(reloaded.name, "Carl");

    Ok(())
}

#[driver_test(id(ID))]
pub async fn create_macro_multiple_fields(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,
        email: String,
    }

    let mut db = test.setup_db(models!(User)).await;

    // Create with multiple fields
    let user = toasty::create!(User, { name: "Carl", email: "carl@example.com" })
        .exec(&mut db)
        .await?;

    assert_eq!(user.name, "Carl");
    assert_eq!(user.email, "carl@example.com");

    Ok(())
}

#[driver_test(id(ID))]
pub async fn create_macro_with_variable(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    let mut db = test.setup_db(models!(User)).await;

    let name = "Carl";

    // Value can be a variable expression
    let user = toasty::create!(User, { name: name }).exec(&mut db).await?;

    assert_eq!(user.name, "Carl");

    Ok(())
}

#[driver_test(id(ID))]
pub async fn create_macro_scoped(test: &mut Test) -> Result<()> {
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

    let mut db = test.setup_db(models!(User, Todo)).await;

    let user = User::create().exec(&mut db).await?;

    // Scoped create — translates to: user.todos().create().title("get something done")
    let todo = toasty::create!(user.todos(), { title: "get something done" })
        .exec(&mut db)
        .await?;

    assert_eq!(todo.title, "get something done");
    assert_eq!(todo.user_id, user.id);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn create_macro_batch(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    let mut db = test.setup_db(models!(User)).await;

    // Batch create — translates to:
    // User::create_many()
    //     .item(User::create().name("Carl"))
    //     .item(User::create().name("Bob"))
    let users = toasty::create!(User, [{ name: "Carl" }, { name: "Bob" }])
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 2);

    let names: Vec<&str> = users.iter().map(|u| u.name.as_str()).collect();
    assert!(names.contains(&"Carl"));
    assert!(names.contains(&"Bob"));

    Ok(())
}

#[driver_test(id(ID))]
pub async fn create_macro_nested_association(test: &mut Test) -> Result<()> {
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
        id: ID,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    // Nested association using plural field name — translates to:
    // User::create().name("Carl").todos([Todo::create().title("get something done")])
    let user = toasty::create!(User, {
        name: "Carl",
        todos: [Todo { title: "get something done" }]
    })
    .exec(&mut db)
    .await?;

    assert_eq!(user.name, "Carl");

    let todos: Vec<_> = user.todos().collect(&mut db).await?;
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].title, "get something done");

    Ok(())
}

#[driver_test(id(ID))]
pub async fn create_macro_nested_multiple(test: &mut Test) -> Result<()> {
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
        id: ID,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    // Multiple nested associations — translates to:
    // User::create()
    //     .name("Carl")
    //     .todos([Todo::create().title("first"), Todo::create().title("second")])
    let user = toasty::create!(User, {
        name: "Carl",
        todos: [Todo { title: "first" }, Todo { title: "second" }]
    })
    .exec(&mut db)
    .await?;

    assert_eq!(user.name, "Carl");

    let mut todos: Vec<_> = user.todos().collect(&mut db).await?;
    assert_eq!(todos.len(), 2);

    todos.sort_by(|a, b| a.title.cmp(&b.title));
    assert_eq!(todos[0].title, "first");
    assert_eq!(todos[1].title, "second");

    Ok(())
}
