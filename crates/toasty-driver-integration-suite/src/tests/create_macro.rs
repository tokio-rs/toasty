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
    let user = toasty::create!(User { name: "Carl" }).exec(&mut db).await?;

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
    let user = toasty::create!(User {
        name: "Carl",
        email: "carl@example.com"
    })
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
    let user = toasty::create!(User { name: name }).exec(&mut db).await?;

    assert_eq!(user.name, "Carl");

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn create_macro_scoped(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = User::create().name("Alice").exec(&mut db).await?;

    // Scoped create — translates to: user.todos().create().title("get something done")
    let todo = toasty::create!(in user.todos() { title: "get something done" })
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

    // Same-type batch — produces a tuple of builders, composed via toasty::batch()
    let (carl, bob) = toasty::batch(toasty::create!(User::[
        { name: "Carl" },
        { name: "Bob" },
    ]))
    .exec(&mut db)
    .await?;

    assert_eq!(carl.name, "Carl");
    assert_eq!(bob.name, "Bob");

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn create_macro_nested_association(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    // Nested association — no type prefix needed; type inferred from field.
    let user = toasty::create!(User {
        name: "Carl",
        todos: [{ title: "get something done" }]
    })
    .exec(&mut db)
    .await?;

    assert_eq!(user.name, "Carl");

    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].title, "get something done");

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn create_macro_nested_multiple(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    // Multiple nested associations
    let user = toasty::create!(User {
        name: "Carl",
        todos: [{ title: "first" }, { title: "second" }]
    })
    .exec(&mut db)
    .await?;

    assert_eq!(user.name, "Carl");

    let mut todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(todos.len(), 2);

    todos.sort_by(|a, b| a.title.cmp(&b.title));
    assert_eq!(todos[0].title, "first");
    assert_eq!(todos[1].title, "second");

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn create_macro_with_belongs_to(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    // Create a todo with an inline belongs_to user
    let todo = toasty::create!(Todo {
        title: "buy milk",
        user: { name: "Carl" }
    })
    .exec(&mut db)
    .await?;

    assert_eq!(todo.title, "buy milk");

    // The user should have been created inline
    let user = User::get_by_id(&mut db, &todo.user_id).await?;
    assert_eq!(user.name, "Carl");

    Ok(())
}

#[driver_test(id(ID))]
pub async fn create_macro_deeply_nested(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        todos: toasty::schema::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::schema::BelongsTo<User>,

        title: String,

        #[has_many]
        tags: toasty::schema::HasMany<Tag>,
    }

    #[derive(Debug, toasty::Model)]
    struct Tag {
        #[key]
        #[auto]
        id: ID,

        #[index]
        todo_id: ID,

        #[belongs_to(key = todo_id, references = id)]
        todo: toasty::schema::BelongsTo<Todo>,

        name: String,
    }

    let mut db = test.setup_db(models!(User, Todo, Tag)).await;

    // Three levels deep: User → Todo → Tag
    let user = toasty::create!(User {
        name: "Carl",
        todos: [{
            title: "get something done",
            tags: [{ name: "urgent" }, { name: "work" }]
        }]
    })
    .exec(&mut db)
    .await?;

    assert_eq!(user.name, "Carl");

    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].title, "get something done");

    let mut tags: Vec<_> = todos[0].tags().exec(&mut db).await?;
    tags.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].name, "urgent");
    assert_eq!(tags[1].name, "work");

    Ok(())
}
