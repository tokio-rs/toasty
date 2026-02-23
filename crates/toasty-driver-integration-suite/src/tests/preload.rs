use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn basic_has_many_and_belongs_to_preload(test: &mut Test) -> Result<()> {
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
        #[allow(dead_code)]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a user with a few todos
    let user = User::create()
        .todo(Todo::create())
        .todo(Todo::create())
        .todo(Todo::create())
        .exec(&db)
        .await?;

    // Find the user, include TODOs
    let user = User::filter_by_id(user.id)
        .include(User::fields().todos())
        .get(&db)
        .await?;

    // This will panic
    assert_eq!(3, user.todos.get().len());

    let id = user.todos.get()[0].id;

    let todo = Todo::filter_by_id(id)
        .include(Todo::fields().user())
        .get(&db)
        .await?;

    assert_eq!(user.id, todo.user.get().id);
    assert_eq!(user.id, todo.user_id);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn multiple_includes_same_model(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[allow(dead_code)]
        name: String,

        #[has_many]
        posts: toasty::HasMany<Post>,

        #[has_many]
        comments: toasty::HasMany<Comment>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        #[allow(dead_code)]
        title: String,

        #[index]
        #[allow(dead_code)]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    #[derive(Debug, toasty::Model)]
    struct Comment {
        #[key]
        #[auto]
        id: ID,

        #[allow(dead_code)]
        text: String,

        #[index]
        #[allow(dead_code)]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let db = test.setup_db(models!(User, Post, Comment)).await;

    // Create a user
    let user = User::create().name("Test User").exec(&db).await?;

    // Create posts associated with the user
    Post::create().title("Post 1").user(&user).exec(&db).await?;

    Post::create().title("Post 2").user(&user).exec(&db).await?;

    // Create comments associated with the user
    Comment::create()
        .text("Comment 1")
        .user(&user)
        .exec(&db)
        .await?;

    Comment::create()
        .text("Comment 2")
        .user(&user)
        .exec(&db)
        .await?;

    Comment::create()
        .text("Comment 3")
        .user(&user)
        .exec(&db)
        .await?;

    // Test individual includes work (baseline)
    let user_with_posts = User::filter_by_id(user.id)
        .include(User::fields().posts())
        .get(&db)
        .await?;
    assert_eq!(2, user_with_posts.posts.get().len());

    let user_with_comments = User::filter_by_id(user.id)
        .include(User::fields().comments())
        .get(&db)
        .await?;
    assert_eq!(3, user_with_comments.comments.get().len());

    // Test multiple includes in one query
    let loaded_user = User::filter_by_id(user.id)
        .include(User::fields().posts()) // First include
        .include(User::fields().comments()) // Second include
        .get(&db)
        .await?;

    assert_eq!(2, loaded_user.posts.get().len());
    assert_eq!(3, loaded_user.comments.get().len());
    Ok(())
}

#[driver_test(id(ID))]
pub async fn basic_has_one_and_belongs_to_preload(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        bio: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    let db = test.setup_db(models!(User, Profile)).await;

    // Create a user with a profile
    let user = User::create()
        .name("John Doe")
        .profile(Profile::create().bio("A person"))
        .exec(&db)
        .await?;

    // Find the user, include profile
    let user = User::filter_by_id(user.id)
        .include(User::fields().profile())
        .get(&db)
        .await?;

    // Verify the profile is preloaded
    let profile = user.profile.get().as_ref().unwrap();
    assert_eq!("A person", profile.bio);
    assert_eq!(user.id, *profile.user_id.as_ref().unwrap());

    let profile_id = profile.id;

    // Test the reciprocal belongs_to preload
    let profile = Profile::filter_by_id(profile_id)
        .include(Profile::fields().user())
        .get(&db)
        .await?;

    assert_eq!(user.id, profile.user.get().as_ref().unwrap().id);
    assert_eq!("John Doe", profile.user.get().as_ref().unwrap().name);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn multiple_includes_with_has_one(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,

        #[has_one]
        settings: toasty::HasOne<Option<Settings>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        bio: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Settings {
        #[key]
        #[auto]
        id: ID,

        theme: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    let db = test.setup_db(models!(User, Profile, Settings)).await;

    // Create a user with both profile and settings
    let user = User::create()
        .name("Jane Doe")
        .profile(Profile::create().bio("Software engineer"))
        .settings(Settings::create().theme("dark"))
        .exec(&db)
        .await?;

    // Test individual includes work (baseline)
    let user_with_profile = User::filter_by_id(user.id)
        .include(User::fields().profile())
        .get(&db)
        .await?;
    assert!(user_with_profile.profile.get().is_some());
    assert_eq!(
        "Software engineer",
        user_with_profile.profile.get().as_ref().unwrap().bio
    );

    let user_with_settings = User::filter_by_id(user.id)
        .include(User::fields().settings())
        .get(&db)
        .await?;
    assert!(user_with_settings.settings.get().is_some());
    assert_eq!(
        "dark",
        user_with_settings.settings.get().as_ref().unwrap().theme
    );

    // Test multiple includes in one query
    let loaded_user = User::filter_by_id(user.id)
        .include(User::fields().profile()) // First include
        .include(User::fields().settings()) // Second include
        .get(&db)
        .await?;

    assert!(loaded_user.profile.get().is_some());
    assert_eq!(
        "Software engineer",
        loaded_user.profile.get().as_ref().unwrap().bio
    );
    assert!(loaded_user.settings.get().is_some());
    assert_eq!("dark", loaded_user.settings.get().as_ref().unwrap().theme);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn combined_has_many_and_has_one_preload(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        bio: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let db = test.setup_db(models!(User, Profile, Todo)).await;

    // Create a user with a profile and multiple todos
    let user = User::create()
        .name("Bob Smith")
        .profile(Profile::create().bio("Developer"))
        .todo(Todo::create().title("Task 1"))
        .todo(Todo::create().title("Task 2"))
        .todo(Todo::create().title("Task 3"))
        .exec(&db)
        .await?;

    // Test combined has_one and has_many preload in a single query
    let loaded_user = User::filter_by_id(user.id)
        .include(User::fields().profile()) // has_one include
        .include(User::fields().todos()) // has_many include
        .get(&db)
        .await?;

    // Verify has_one association is preloaded
    assert!(loaded_user.profile.get().is_some());
    assert_eq!("Developer", loaded_user.profile.get().as_ref().unwrap().bio);

    // Verify has_many association is preloaded
    assert_eq!(3, loaded_user.todos.get().len());
    let todo_titles: Vec<&str> = loaded_user
        .todos
        .get()
        .iter()
        .map(|t| t.title.as_str())
        .collect();
    assert!(todo_titles.contains(&"Task 1"));
    assert!(todo_titles.contains(&"Task 2"));
    assert!(todo_titles.contains(&"Task 3"));
    Ok(())
}

#[driver_test(id(ID))]
pub async fn preload_on_empty_table(test: &mut Test) -> Result<()> {
    if !test.capability().sql {
        return Ok(());
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        #[allow(dead_code)]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        #[allow(dead_code)]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        #[allow(dead_code)]
        user: toasty::BelongsTo<User>,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Query with include on empty table - should return empty result, not SQL error
    let users: Vec<User> = User::all()
        .include(User::fields().todos())
        .collect(&db)
        .await?;

    assert_eq!(0, users.len());
    Ok(())
}

#[driver_test(id(ID))]
pub async fn preload_on_empty_query(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[index]
        #[allow(dead_code)]
        name: String,

        #[has_many]
        #[allow(dead_code)]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        #[allow(dead_code)]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        #[allow(dead_code)]
        user: toasty::BelongsTo<User>,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Query with include on empty table - should return empty result, not SQL error
    let users: Vec<User> = User::filter_by_name("foo")
        .include(User::fields().todos())
        .collect(&db)
        .await?;

    assert_eq!(0, users.len());
    Ok(())
}

#[driver_test(id(ID))]
pub async fn nested_has_many_preload(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[has_many]
        steps: toasty::HasMany<Step>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Step {
        #[key]
        #[auto]
        id: ID,

        description: String,

        #[index]
        todo_id: ID,

        #[belongs_to(key = todo_id, references = id)]
        todo: toasty::BelongsTo<Todo>,
    }

    let db = test.setup_db(models!(User, Todo, Step)).await;

    // Create a user with todos, each with steps
    let user = User::create()
        .name("Alice")
        .todo(
            Todo::create()
                .title("Todo 1")
                .step(Step::create().description("Step 1a"))
                .step(Step::create().description("Step 1b")),
        )
        .todo(
            Todo::create()
                .title("Todo 2")
                .step(Step::create().description("Step 2a"))
                .step(Step::create().description("Step 2b"))
                .step(Step::create().description("Step 2c")),
        )
        .exec(&db)
        .await
        .unwrap();

    // Load user with nested include: todos AND their steps
    let user = User::filter_by_id(user.id)
        .include(User::fields().todos().steps())
        .get(&db)
        .await
        .unwrap();

    // Verify todos are loaded
    let todos = user.todos.get();
    assert_eq!(2, todos.len());

    // Verify steps are loaded on each todo
    let mut all_step_descriptions: Vec<&str> = Vec::new();
    for todo in todos {
        let steps = todo.steps.get();
        for step in steps {
            all_step_descriptions.push(&step.description);
        }
    }
    all_step_descriptions.sort();
    assert_eq!(
        all_step_descriptions,
        vec!["Step 1a", "Step 1b", "Step 2a", "Step 2b", "Step 2c"]
    );

    // Verify per-todo step distribution: one todo has 2 steps, the other has 3
    let mut step_counts: Vec<usize> = todos.iter().map(|t| t.steps.get().len()).collect();
    step_counts.sort();
    assert_eq!(step_counts, vec![2, 3]);
}

#[driver_test(id(ID))]
pub async fn nested_preload_on_collection(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[has_many]
        steps: toasty::HasMany<Step>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Step {
        #[key]
        #[auto]
        id: ID,

        description: String,

        #[index]
        todo_id: ID,

        #[belongs_to(key = todo_id, references = id)]
        todo: toasty::BelongsTo<Todo>,
    }

    let db = test.setup_db(models!(User, Todo, Step)).await;

    // Alice: 2 todos, with 2 and 1 steps respectively
    User::create()
        .name("Alice")
        .todo(
            Todo::create()
                .title("Todo A1")
                .step(Step::create().description("Step A1a"))
                .step(Step::create().description("Step A1b")),
        )
        .todo(
            Todo::create()
                .title("Todo A2")
                .step(Step::create().description("Step A2a")),
        )
        .exec(&db)
        .await?;

    // Bob: 1 todo with 3 steps
    User::create()
        .name("Bob")
        .todo(
            Todo::create()
                .title("Todo B1")
                .step(Step::create().description("Step B1a"))
                .step(Step::create().description("Step B1b"))
                .step(Step::create().description("Step B1c")),
        )
        .exec(&db)
        .await?;

    // Carol: no todos
    User::create().name("Carol").exec(&db).await?;

    let mut users: Vec<User> = User::all()
        .include(User::fields().todos().steps())
        .collect(&db)
        .await?;

    assert_eq!(3, users.len());

    users.sort_by_key(|u| u.name.clone());

    let alice = &users[0];
    assert_eq!("Alice", alice.name);
    let alice_todos = alice.todos.get();
    assert_eq!(2, alice_todos.len());
    let mut alice_step_counts: Vec<usize> =
        alice_todos.iter().map(|t| t.steps.get().len()).collect();
    alice_step_counts.sort();
    assert_eq!(vec![1, 2], alice_step_counts);

    let bob = &users[1];
    assert_eq!("Bob", bob.name);
    let bob_todos = bob.todos.get();
    assert_eq!(1, bob_todos.len());
    assert_eq!(3, bob_todos[0].steps.get().len());

    let carol = &users[2];
    assert_eq!("Carol", carol.name);
    assert_eq!(0, carol.todos.get().len());

    Ok(())
}

#[driver_test(id(ID))]
pub async fn nested_preload_empty_intermediate(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[has_many]
        steps: toasty::HasMany<Step>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Step {
        #[key]
        #[auto]
        id: ID,

        description: String,

        #[index]
        todo_id: ID,

        #[belongs_to(key = todo_id, references = id)]
        todo: toasty::BelongsTo<Todo>,
    }

    let db = test.setup_db(models!(User, Todo, Step)).await;

    let user = User::create()
        .name("Alice")
        .todo(
            Todo::create()
                .title("Has steps")
                .step(Step::create().description("Step 1"))
                .step(Step::create().description("Step 2")),
        )
        .todo(Todo::create().title("No steps"))
        .exec(&db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().todos().steps())
        .get(&db)
        .await?;

    let todos = user.todos.get();
    assert_eq!(2, todos.len());

    let mut todos: Vec<&Todo> = todos.iter().collect();
    todos.sort_by_key(|t| t.title.as_str());

    // "Has steps" < "No steps"
    assert_eq!("Has steps", todos[0].title);
    assert_eq!(2, todos[0].steps.get().len());

    assert_eq!("No steps", todos[1].title);
    assert_eq!(0, todos[1].steps.get().len());

    Ok(())
}

#[driver_test(id(ID))]
pub async fn three_level_deep_preload(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[has_many]
        steps: toasty::HasMany<Step>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Step {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        todo_id: ID,

        #[belongs_to(key = todo_id, references = id)]
        todo: toasty::BelongsTo<Todo>,

        #[has_many]
        notes: toasty::HasMany<Note>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Note {
        #[key]
        #[auto]
        id: ID,

        text: String,

        #[index]
        step_id: ID,

        #[belongs_to(key = step_id, references = id)]
        step: toasty::BelongsTo<Step>,
    }

    let db = test.setup_db(models!(User, Todo, Step, Note)).await;

    let user = User::create()
        .name("Alice")
        .todo(
            Todo::create()
                .title("Todo 1")
                .step(
                    Step::create()
                        .title("Step 1a")
                        .note(Note::create().text("Note 1a-i"))
                        .note(Note::create().text("Note 1a-ii")),
                )
                .step(
                    Step::create()
                        .title("Step 1b")
                        .note(Note::create().text("Note 1b-i")),
                ),
        )
        .exec(&db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().todos().steps().notes())
        .get(&db)
        .await?;

    let todos = user.todos.get();
    assert_eq!(1, todos.len());

    let steps = todos[0].steps.get();
    assert_eq!(2, steps.len());

    let total_notes: usize = steps.iter().map(|s| s.notes.get().len()).sum();
    assert_eq!(3, total_notes);

    // Per-step note counts
    let mut note_counts: Vec<usize> = steps.iter().map(|s| s.notes.get().len()).collect();
    note_counts.sort();
    assert_eq!(vec![1, 2], note_counts);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn nested_and_shallow_includes(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        bio: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[has_many]
        steps: toasty::HasMany<Step>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Step {
        #[key]
        #[auto]
        id: ID,

        description: String,

        #[index]
        todo_id: ID,

        #[belongs_to(key = todo_id, references = id)]
        todo: toasty::BelongsTo<Todo>,
    }

    let db = test.setup_db(models!(User, Profile, Todo, Step)).await;

    let user = User::create()
        .name("Alice")
        .profile(Profile::create().bio("Developer"))
        .todo(
            Todo::create()
                .title("Todo 1")
                .step(Step::create().description("Step 1a"))
                .step(Step::create().description("Step 1b")),
        )
        .todo(
            Todo::create()
                .title("Todo 2")
                .step(Step::create().description("Step 2a")),
        )
        .exec(&db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().todos().steps()) // nested
        .include(User::fields().profile()) // shallow
        .get(&db)
        .await?;

    // Verify shallow include
    let profile = user.profile.get().as_ref().unwrap();
    assert_eq!("Developer", profile.bio);

    // Verify nested include
    let todos = user.todos.get();
    assert_eq!(2, todos.len());
    let total_steps: usize = todos.iter().map(|t| t.steps.get().len()).sum();
    assert_eq!(3, total_steps);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn two_independent_nested_paths(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,

        #[has_many]
        posts: toasty::HasMany<Post>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[has_many]
        steps: toasty::HasMany<Step>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Step {
        #[key]
        #[auto]
        id: ID,

        description: String,

        #[index]
        todo_id: ID,

        #[belongs_to(key = todo_id, references = id)]
        todo: toasty::BelongsTo<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[has_many]
        comments: toasty::HasMany<Comment>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Comment {
        #[key]
        #[auto]
        id: ID,

        text: String,

        #[index]
        post_id: ID,

        #[belongs_to(key = post_id, references = id)]
        post: toasty::BelongsTo<Post>,
    }

    let db = test.setup_db(models!(User, Todo, Step, Post, Comment)).await;

    let user = User::create()
        .name("Alice")
        .todo(
            Todo::create()
                .title("Todo 1")
                .step(Step::create().description("Step 1a"))
                .step(Step::create().description("Step 1b")),
        )
        .post(
            Post::create()
                .title("Post 1")
                .comment(Comment::create().text("Comment 1"))
                .comment(Comment::create().text("Comment 2"))
                .comment(Comment::create().text("Comment 3")),
        )
        .exec(&db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().todos().steps())
        .include(User::fields().posts().comments())
        .get(&db)
        .await?;

    let todos = user.todos.get();
    assert_eq!(1, todos.len());
    assert_eq!(2, todos[0].steps.get().len());

    let posts = user.posts.get();
    assert_eq!(1, posts.len());
    assert_eq!(3, posts[0].comments.get().len());

    Ok(())
}

#[driver_test(id(ID))]
pub async fn nested_has_many_to_has_one(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[has_one]
        attachment: toasty::HasOne<Option<Attachment>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Attachment {
        #[key]
        #[auto]
        id: ID,

        content: String,

        #[unique]
        todo_id: Option<ID>,

        #[belongs_to(key = todo_id, references = id)]
        todo: toasty::BelongsTo<Option<Todo>>,
    }

    let db = test.setup_db(models!(User, Todo, Attachment)).await;

    let user = User::create()
        .name("Alice")
        .todo(
            Todo::create()
                .title("With attachment")
                .attachment(Attachment::create().content("file.pdf")),
        )
        .todo(Todo::create().title("Without attachment"))
        .exec(&db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().todos().attachment())
        .get(&db)
        .await?;

    let todos = user.todos.get();
    assert_eq!(2, todos.len());

    let mut todos: Vec<&Todo> = todos.iter().collect();
    todos.sort_by_key(|t| t.title.as_str());

    // "With attachment" < "Without attachment"
    assert_eq!("With attachment", todos[0].title);
    assert_eq!("file.pdf", todos[0].attachment.get().as_ref().unwrap().content);

    assert_eq!("Without attachment", todos[1].title);
    assert!(todos[1].attachment.get().is_none());

    Ok(())
}

#[driver_test(id(ID))]
pub async fn nested_has_one_to_has_many(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        bio: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        #[has_many]
        skills: toasty::HasMany<Skill>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Skill {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[index]
        profile_id: ID,

        #[belongs_to(key = profile_id, references = id)]
        profile: toasty::BelongsTo<Profile>,
    }

    let db = test.setup_db(models!(User, Profile, Skill)).await;

    // User with a profile that has skills
    let alice = User::create()
        .name("Alice")
        .profile(
            Profile::create()
                .bio("Developer")
                .skill(Skill::create().name("Rust"))
                .skill(Skill::create().name("Go")),
        )
        .exec(&db)
        .await?;

    // User without a profile
    let bob = User::create().name("Bob").exec(&db).await?;

    // Load user with profile and nested skills
    let alice = User::filter_by_id(alice.id)
        .include(User::fields().profile().skills())
        .get(&db)
        .await?;

    let profile = alice.profile.get().as_ref().unwrap();
    assert_eq!("Developer", profile.bio);
    let skills = profile.skills.get();
    assert_eq!(2, skills.len());
    let mut skill_names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
    skill_names.sort();
    assert_eq!(vec!["Go", "Rust"], skill_names);

    // Load user without profile - should not panic
    let bob = User::filter_by_id(bob.id)
        .include(User::fields().profile().skills())
        .get(&db)
        .await?;

    assert!(bob.profile.get().is_none());

    Ok(())
}
