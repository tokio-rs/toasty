use crate::prelude::*;

/// Tests that preloading a `HasOne<Option<_>>` correctly distinguishes between
/// "not loaded" and "loaded as None" when the relation does not exist.
#[driver_test(id(ID), scenario(crate::scenarios::has_one_optional_belongs_to))]
pub async fn preload_has_one_option_none_then_some(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    // Create a user WITHOUT a profile
    let user_no_profile = User::create().name("No Profile").exec(&mut db).await?;

    // Preload the profile — no profile exists, so it should be `None` (loaded)
    let user_no_profile = User::filter_by_id(user_no_profile.id)
        .include(User::fields().profile())
        .get(&mut db)
        .await?;

    // `.get()` must not panic — the relation was preloaded and is None
    assert!(user_no_profile.profile.get().is_none());

    // Create a user WITH a profile
    let user_with_profile = User::create()
        .name("Has Profile")
        .profile(Profile::create().bio("A bio"))
        .exec(&mut db)
        .await?;

    // Preload the profile — a profile exists, so it should be `Some`
    let user_with_profile = User::filter_by_id(user_with_profile.id)
        .include(User::fields().profile())
        .get(&mut db)
        .await?;

    let profile = user_with_profile.profile.get().as_ref().unwrap();
    assert_eq!("A bio", profile.bio);
    assert_eq!(user_with_profile.id, *profile.user_id.as_ref().unwrap());

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn basic_has_many_and_belongs_to_preload(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    // Create a user with a few todos
    let user = User::create()
        .name("Alice")
        .todo(Todo::create().title("todo 1"))
        .todo(Todo::create().title("todo 2"))
        .todo(Todo::create().title("todo 3"))
        .exec(&mut db)
        .await?;

    // Find the user, include TODOs
    let user = User::filter_by_id(user.id)
        .include(User::fields().todos())
        .get(&mut db)
        .await?;

    // This will panic
    assert_eq!(3, user.todos.get().len());

    let id = user.todos.get()[0].id;

    let todo = Todo::filter_by_id(id)
        .include(Todo::fields().user())
        .get(&mut db)
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

    let mut db = test.setup_db(models!(User, Post, Comment)).await;

    // Create a user
    let user = User::create().name("Test User").exec(&mut db).await?;

    // Create posts associated with the user
    Post::create()
        .title("Post 1")
        .user(&user)
        .exec(&mut db)
        .await?;

    Post::create()
        .title("Post 2")
        .user(&user)
        .exec(&mut db)
        .await?;

    // Create comments associated with the user
    Comment::create()
        .text("Comment 1")
        .user(&user)
        .exec(&mut db)
        .await?;

    Comment::create()
        .text("Comment 2")
        .user(&user)
        .exec(&mut db)
        .await?;

    Comment::create()
        .text("Comment 3")
        .user(&user)
        .exec(&mut db)
        .await?;

    // Test individual includes work (baseline)
    let user_with_posts = User::filter_by_id(user.id)
        .include(User::fields().posts())
        .get(&mut db)
        .await?;
    assert_eq!(2, user_with_posts.posts.get().len());

    let user_with_comments = User::filter_by_id(user.id)
        .include(User::fields().comments())
        .get(&mut db)
        .await?;
    assert_eq!(3, user_with_comments.comments.get().len());

    // Test multiple includes in one query
    let loaded_user = User::filter_by_id(user.id)
        .include(User::fields().posts()) // First include
        .include(User::fields().comments()) // Second include
        .get(&mut db)
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

    let mut db = test.setup_db(models!(User, Profile)).await;

    // Create a user with a profile
    let user = User::create()
        .name("John Doe")
        .profile(Profile::create().bio("A person"))
        .exec(&mut db)
        .await?;

    // Find the user, include profile
    let user = User::filter_by_id(user.id)
        .include(User::fields().profile())
        .get(&mut db)
        .await?;

    // Verify the profile is preloaded
    let profile = user.profile.get().as_ref().unwrap();
    assert_eq!("A person", profile.bio);
    assert_eq!(user.id, *profile.user_id.as_ref().unwrap());

    let profile_id = profile.id;

    // Test the reciprocal belongs_to preload
    let profile = Profile::filter_by_id(profile_id)
        .include(Profile::fields().user())
        .get(&mut db)
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

    let mut db = test.setup_db(models!(User, Profile, Settings)).await;

    // Create a user with both profile and settings
    let user = User::create()
        .name("Jane Doe")
        .profile(Profile::create().bio("Software engineer"))
        .settings(Settings::create().theme("dark"))
        .exec(&mut db)
        .await?;

    // Test individual includes work (baseline)
    let user_with_profile = User::filter_by_id(user.id)
        .include(User::fields().profile())
        .get(&mut db)
        .await?;
    assert!(user_with_profile.profile.get().is_some());
    assert_eq!(
        "Software engineer",
        user_with_profile.profile.get().as_ref().unwrap().bio
    );

    let user_with_settings = User::filter_by_id(user.id)
        .include(User::fields().settings())
        .get(&mut db)
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
        .get(&mut db)
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

    let mut db = test.setup_db(models!(User, Profile, Todo)).await;

    // Create a user with a profile and multiple todos
    let user = User::create()
        .name("Bob Smith")
        .profile(Profile::create().bio("Developer"))
        .todo(Todo::create().title("Task 1"))
        .todo(Todo::create().title("Task 2"))
        .todo(Todo::create().title("Task 3"))
        .exec(&mut db)
        .await?;

    // Test combined has_one and has_many preload in a single query
    let loaded_user = User::filter_by_id(user.id)
        .include(User::fields().profile()) // has_one include
        .include(User::fields().todos()) // has_many include
        .get(&mut db)
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

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_on_empty_table(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    // Query with include on empty table - should return empty result, not SQL error
    let users: Vec<User> = User::all()
        .include(User::fields().todos())
        .exec(&mut db)
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

    let mut db = test.setup_db(models!(User, Todo)).await;

    // Query with include on empty table - should return empty result, not SQL error
    let users: Vec<User> = User::filter_by_name("foo")
        .include(User::fields().todos())
        .exec(&mut db)
        .await?;

    assert_eq!(0, users.len());
    Ok(())
}

/// HasMany<T> + BelongsTo<Option<T>>: nullable FK allows children to exist
/// without a parent. Tests preloading from both directions.
#[driver_test(id(ID))]
pub async fn preload_has_many_with_optional_belongs_to(test: &mut Test) -> Result<()> {
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
        title: String,

        #[index]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    // Create a user with linked todos
    let user = User::create()
        .name("Alice")
        .todo(Todo::create().title("Task 1"))
        .todo(Todo::create().title("Task 2"))
        .exec(&mut db)
        .await?;

    // Preload HasMany from parent side
    let user = User::filter_by_id(user.id)
        .include(User::fields().todos())
        .get(&mut db)
        .await?;

    assert_eq!(2, user.todos.get().len());

    let todo_id = user.todos.get()[0].id;

    // Preload BelongsTo<Option<User>> from child side — linked todo
    let todo = Todo::filter_by_id(todo_id)
        .include(Todo::fields().user())
        .get(&mut db)
        .await?;

    assert_eq!(user.id, todo.user.get().as_ref().unwrap().id);

    // Create an orphan todo (no user)
    let orphan = Todo::create().title("Orphan").exec(&mut db).await?;

    // Preload BelongsTo<Option<User>> on orphan — should be None
    let orphan = Todo::filter_by_id(orphan.id)
        .include(Todo::fields().user())
        .get(&mut db)
        .await?;

    assert!(orphan.user.get().is_none());

    Ok(())
}

/// HasOne<Option<T>> + BelongsTo<T> (required FK): the child always points to a
/// parent, but the parent may or may not have a child. Tests preloading from
/// both directions.
#[driver_test(id(ID))]
pub async fn preload_has_one_optional_with_required_belongs_to(test: &mut Test) -> Result<()> {
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
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let mut db = test.setup_db(models!(User, Profile)).await;

    // Create a user WITH a profile
    let user_with = User::create()
        .name("Has Profile")
        .profile(Profile::create().bio("hello"))
        .exec(&mut db)
        .await?;

    // Create a user WITHOUT a profile
    let user_without = User::create().name("No Profile").exec(&mut db).await?;

    // Preload HasOne<Option<Profile>> — profile exists
    let loaded = User::filter_by_id(user_with.id)
        .include(User::fields().profile())
        .get(&mut db)
        .await?;

    let profile = loaded.profile.get().as_ref().unwrap();
    assert_eq!("hello", profile.bio);
    assert_eq!(user_with.id, profile.user_id);

    // Preload HasOne<Option<Profile>> — no profile
    let loaded = User::filter_by_id(user_without.id)
        .include(User::fields().profile())
        .get(&mut db)
        .await?;

    assert!(loaded.profile.get().is_none());

    // Preload BelongsTo<User> (required) from child side
    let profile = Profile::filter_by_user_id(user_with.id)
        .include(Profile::fields().user())
        .get(&mut db)
        .await?;

    assert_eq!(user_with.id, profile.user.get().id);
    assert_eq!("Has Profile", profile.user.get().name);

    Ok(())
}

/// HasOne<T> (required) + BelongsTo<Option<T>>: creating a parent requires
/// providing a child, but the child FK is nullable. Tests preloading from both
/// directions.
#[driver_test(id(ID))]
pub async fn preload_has_one_required_with_optional_belongs_to(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        profile: toasty::HasOne<Profile>,
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

    let mut db = test.setup_db(models!(User, Profile)).await;

    // Create a user (must provide a profile since HasOne<T> is required)
    let user = User::create()
        .name("Alice")
        .profile(Profile::create().bio("a bio"))
        .exec(&mut db)
        .await?;

    // Preload HasOne<Profile> (required) from parent side
    let loaded = User::filter_by_id(user.id)
        .include(User::fields().profile())
        .get(&mut db)
        .await?;

    let profile = loaded.profile.get();
    assert_eq!("a bio", profile.bio);
    assert_eq!(user.id, *profile.user_id.as_ref().unwrap());

    // Preload BelongsTo<Option<User>> from child side
    let profile = Profile::filter_by_id(profile.id)
        .include(Profile::fields().user())
        .get(&mut db)
        .await?;

    assert_eq!(user.id, profile.user.get().as_ref().unwrap().id);
    assert_eq!("Alice", profile.user.get().as_ref().unwrap().name);

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

    let mut db = test.setup_db(models!(User, Todo, Step)).await;

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
        .exec(&mut db)
        .await
        .unwrap();

    // Load user with nested include: todos AND their steps
    let user = User::filter_by_id(user.id)
        .include(User::fields().todos().steps())
        .get(&mut db)
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
}

// ===== HasMany -> HasOne<Option<T>> =====
// User has_many Posts, each Post has_one optional Detail
#[driver_test(id(ID))]
pub async fn nested_has_many_then_has_one_optional(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        posts: toasty::HasMany<Post>,
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

        #[has_one]
        detail: toasty::HasOne<Option<Detail>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Detail {
        #[key]
        #[auto]
        id: ID,

        body: String,

        #[unique]
        post_id: Option<ID>,

        #[belongs_to(key = post_id, references = id)]
        post: toasty::BelongsTo<Option<Post>>,
    }

    let mut db = test.setup_db(models!(User, Post, Detail)).await;

    let user = User::create()
        .name("Alice")
        .post(
            Post::create()
                .title("P1")
                .detail(Detail::create().body("D1")),
        )
        .post(Post::create().title("P2")) // no detail
        .exec(&mut db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().posts().detail())
        .get(&mut db)
        .await?;

    let posts = user.posts.get();
    assert_eq!(2, posts.len());

    let mut with_detail = 0;
    let mut without_detail = 0;
    for post in posts {
        match post.detail.get() {
            Some(d) => {
                assert_eq!("D1", d.body);
                with_detail += 1;
            }
            None => without_detail += 1,
        }
    }
    assert_eq!(1, with_detail);
    assert_eq!(1, without_detail);

    Ok(())
}

// ===== HasMany -> HasOne<T> (required) =====
// User has_many Accounts, each Account has_one required Settings
#[driver_test(id(ID))]
pub async fn nested_has_many_then_has_one_required(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        accounts: toasty::HasMany<Account>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Account {
        #[key]
        #[auto]
        id: ID,

        label: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[has_one]
        settings: toasty::HasOne<Settings>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Settings {
        #[key]
        #[auto]
        id: ID,

        theme: String,

        #[unique]
        account_id: Option<ID>,

        #[belongs_to(key = account_id, references = id)]
        account: toasty::BelongsTo<Option<Account>>,
    }

    let mut db = test.setup_db(models!(User, Account, Settings)).await;

    let user = User::create()
        .name("Bob")
        .account(
            Account::create()
                .label("A1")
                .settings(Settings::create().theme("dark")),
        )
        .account(
            Account::create()
                .label("A2")
                .settings(Settings::create().theme("light")),
        )
        .exec(&mut db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().accounts().settings())
        .get(&mut db)
        .await?;

    let accounts = user.accounts.get();
    assert_eq!(2, accounts.len());

    let mut themes: Vec<&str> = accounts
        .iter()
        .map(|a| a.settings.get().theme.as_str())
        .collect();
    themes.sort();
    assert_eq!(themes, vec!["dark", "light"]);

    Ok(())
}

// ===== HasMany -> BelongsTo<T> (required) =====
// Category has_many Items, each Item belongs_to a Brand
#[driver_test(id(ID))]
pub async fn nested_has_many_then_belongs_to_required(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Category {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        items: toasty::HasMany<Item>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Brand {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        category_id: ID,

        #[belongs_to(key = category_id, references = id)]
        category: toasty::BelongsTo<Category>,

        #[index]
        brand_id: ID,

        #[belongs_to(key = brand_id, references = id)]
        brand: toasty::BelongsTo<Brand>,
    }

    let mut db = test.setup_db(models!(Category, Brand, Item)).await;

    let brand_a = Brand::create().name("BrandA").exec(&mut db).await?;
    let brand_b = Brand::create().name("BrandB").exec(&mut db).await?;

    let cat = Category::create()
        .name("Electronics")
        .item(Item::create().title("Phone").brand(&brand_a))
        .item(Item::create().title("Laptop").brand(&brand_b))
        .exec(&mut db)
        .await?;

    let cat = Category::filter_by_id(cat.id)
        .include(Category::fields().items().brand())
        .get(&mut db)
        .await?;

    let items = cat.items.get();
    assert_eq!(2, items.len());

    let mut brand_names: Vec<&str> = items.iter().map(|i| i.brand.get().name.as_str()).collect();
    brand_names.sort();
    assert_eq!(brand_names, vec!["BrandA", "BrandB"]);

    Ok(())
}

// ===== HasMany -> BelongsTo<Option<T>> =====
// Team has_many Tasks, each Task optionally belongs_to an Assignee
#[driver_test(id(ID))]
pub async fn nested_has_many_then_belongs_to_optional(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Team {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        tasks: toasty::HasMany<Task>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Assignee {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Task {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        team_id: ID,

        #[belongs_to(key = team_id, references = id)]
        team: toasty::BelongsTo<Team>,

        #[index]
        assignee_id: Option<ID>,

        #[belongs_to(key = assignee_id, references = id)]
        assignee: toasty::BelongsTo<Option<Assignee>>,
    }

    let mut db = test.setup_db(models!(Team, Assignee, Task)).await;

    let person = Assignee::create().name("Alice").exec(&mut db).await?;

    let team = Team::create()
        .name("Engineering")
        .task(Task::create().title("Assigned").assignee(&person))
        .task(Task::create().title("Unassigned"))
        .exec(&mut db)
        .await?;

    let team = Team::filter_by_id(team.id)
        .include(Team::fields().tasks().assignee())
        .get(&mut db)
        .await?;

    let tasks = team.tasks.get();
    assert_eq!(2, tasks.len());

    let mut assigned = 0;
    let mut unassigned = 0;
    for task in tasks {
        match task.assignee.get() {
            Some(a) => {
                assert_eq!("Alice", a.name);
                assigned += 1;
            }
            None => unassigned += 1,
        }
    }
    assert_eq!(1, assigned);
    assert_eq!(1, unassigned);

    Ok(())
}

// ===== HasOne<Option<T>> -> HasMany =====
// User has_one optional Profile, Profile has_many Badges
#[driver_test(id(ID))]
pub async fn nested_has_one_optional_then_has_many(test: &mut Test) -> Result<()> {
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
        badges: toasty::HasMany<Badge>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Badge {
        #[key]
        #[auto]
        id: ID,

        label: String,

        #[index]
        profile_id: ID,

        #[belongs_to(key = profile_id, references = id)]
        profile: toasty::BelongsTo<Profile>,
    }

    let mut db = test.setup_db(models!(User, Profile, Badge)).await;

    // User with profile and badges
    let user = User::create()
        .name("Alice")
        .profile(
            Profile::create()
                .bio("hi")
                .badge(Badge::create().label("Gold"))
                .badge(Badge::create().label("Silver")),
        )
        .exec(&mut db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().profile().badges())
        .get(&mut db)
        .await?;

    let profile = user.profile.get().as_ref().unwrap();
    assert_eq!("hi", profile.bio);
    let mut labels: Vec<&str> = profile
        .badges
        .get()
        .iter()
        .map(|b| b.label.as_str())
        .collect();
    labels.sort();
    assert_eq!(labels, vec!["Gold", "Silver"]);

    // User without profile - nested preload should handle gracefully
    let user2 = User::create().name("Bob").exec(&mut db).await?;

    let user2 = User::filter_by_id(user2.id)
        .include(User::fields().profile().badges())
        .get(&mut db)
        .await?;

    assert!(user2.profile.get().is_none());

    Ok(())
}

// ===== HasOne<T> (required) -> HasMany =====
// Order has_one required Invoice, Invoice has_many LineItems
#[driver_test(id(ID))]
pub async fn nested_has_one_required_then_has_many(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Order {
        #[key]
        #[auto]
        id: ID,

        label: String,

        #[has_one]
        invoice: toasty::HasOne<Invoice>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Invoice {
        #[key]
        #[auto]
        id: ID,

        code: String,

        #[unique]
        order_id: Option<ID>,

        #[belongs_to(key = order_id, references = id)]
        order: toasty::BelongsTo<Option<Order>>,

        #[has_many]
        line_items: toasty::HasMany<LineItem>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct LineItem {
        #[key]
        #[auto]
        id: ID,

        description: String,

        #[index]
        invoice_id: ID,

        #[belongs_to(key = invoice_id, references = id)]
        invoice: toasty::BelongsTo<Invoice>,
    }

    let mut db = test.setup_db(models!(Order, Invoice, LineItem)).await;

    let order = Order::create()
        .label("Order1")
        .invoice(
            Invoice::create()
                .code("INV-001")
                .line_item(LineItem::create().description("Widget"))
                .line_item(LineItem::create().description("Gadget")),
        )
        .exec(&mut db)
        .await?;

    let order = Order::filter_by_id(order.id)
        .include(Order::fields().invoice().line_items())
        .get(&mut db)
        .await?;

    let invoice = order.invoice.get();
    assert_eq!("INV-001", invoice.code);
    let mut descs: Vec<&str> = invoice
        .line_items
        .get()
        .iter()
        .map(|li| li.description.as_str())
        .collect();
    descs.sort();
    assert_eq!(descs, vec!["Gadget", "Widget"]);

    Ok(())
}

// ===== HasOne<Option<T>> -> HasOne<Option<T>> =====
// User has_one optional Profile, Profile has_one optional Avatar
#[driver_test(id(ID))]
pub async fn nested_has_one_optional_then_has_one_optional(test: &mut Test) -> Result<()> {
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

        #[has_one]
        avatar: toasty::HasOne<Option<Avatar>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Avatar {
        #[key]
        #[auto]
        id: ID,

        url: String,

        #[unique]
        profile_id: Option<ID>,

        #[belongs_to(key = profile_id, references = id)]
        profile: toasty::BelongsTo<Option<Profile>>,
    }

    let mut db = test.setup_db(models!(User, Profile, Avatar)).await;

    // User -> Profile -> Avatar (all present)
    let user = User::create()
        .name("Alice")
        .profile(
            Profile::create()
                .bio("hi")
                .avatar(Avatar::create().url("pic.png")),
        )
        .exec(&mut db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().profile().avatar())
        .get(&mut db)
        .await?;

    let profile = user.profile.get().as_ref().unwrap();
    assert_eq!("hi", profile.bio);
    let avatar = profile.avatar.get().as_ref().unwrap();
    assert_eq!("pic.png", avatar.url);

    // User -> Profile (present) -> Avatar (missing)
    let user2 = User::create()
        .name("Bob")
        .profile(Profile::create().bio("no pic"))
        .exec(&mut db)
        .await?;

    let user2 = User::filter_by_id(user2.id)
        .include(User::fields().profile().avatar())
        .get(&mut db)
        .await?;

    let profile2 = user2.profile.get().as_ref().unwrap();
    assert_eq!("no pic", profile2.bio);
    assert!(profile2.avatar.get().is_none());

    // User -> Profile (missing) - nested preload short-circuits
    let user3 = User::create().name("Carol").exec(&mut db).await?;

    let user3 = User::filter_by_id(user3.id)
        .include(User::fields().profile().avatar())
        .get(&mut db)
        .await?;

    assert!(user3.profile.get().is_none());

    Ok(())
}

// ===== HasOne<T> (required) -> HasOne<T> (required) =====
// User has_one required Profile, Profile has_one required Avatar
#[driver_test(id(ID))]
pub async fn nested_has_one_required_then_has_one_required(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        profile: toasty::HasOne<Profile>,
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

        #[has_one]
        avatar: toasty::HasOne<Avatar>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Avatar {
        #[key]
        #[auto]
        id: ID,

        url: String,

        #[unique]
        profile_id: Option<ID>,

        #[belongs_to(key = profile_id, references = id)]
        profile: toasty::BelongsTo<Option<Profile>>,
    }

    let mut db = test.setup_db(models!(User, Profile, Avatar)).await;

    let user = User::create()
        .name("Alice")
        .profile(
            Profile::create()
                .bio("engineer")
                .avatar(Avatar::create().url("alice.jpg")),
        )
        .exec(&mut db)
        .await?;

    let user = User::filter_by_id(user.id)
        .include(User::fields().profile().avatar())
        .get(&mut db)
        .await?;

    let profile = user.profile.get();
    assert_eq!("engineer", profile.bio);
    let avatar = profile.avatar.get();
    assert_eq!("alice.jpg", avatar.url);

    Ok(())
}

// ===== HasOne<Option<T>> -> BelongsTo<T> (required) =====
// User has_one optional Review, Review belongs_to a Product
#[driver_test(id(ID))]
pub async fn nested_has_one_optional_then_belongs_to_required(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        review: toasty::HasOne<Option<Review>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Product {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Review {
        #[key]
        #[auto]
        id: ID,

        body: String,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        #[index]
        product_id: ID,

        #[belongs_to(key = product_id, references = id)]
        product: toasty::BelongsTo<Product>,
    }

    let mut db = test.setup_db(models!(User, Product, Review)).await;

    let product = Product::create().name("Widget").exec(&mut db).await?;

    let user = User::create()
        .name("Alice")
        .review(Review::create().body("Great!").product(&product))
        .exec(&mut db)
        .await?;

    // User with review -> preload nested product
    let user = User::filter_by_id(user.id)
        .include(User::fields().review().product())
        .get(&mut db)
        .await?;

    let review = user.review.get().as_ref().unwrap();
    assert_eq!("Great!", review.body);
    assert_eq!("Widget", review.product.get().name);

    // User without review
    let user2 = User::create().name("Bob").exec(&mut db).await?;

    let user2 = User::filter_by_id(user2.id)
        .include(User::fields().review().product())
        .get(&mut db)
        .await?;

    assert!(user2.review.get().is_none());

    Ok(())
}

// ===== BelongsTo<T> (required) -> HasMany =====
// Comment belongs_to a Post, Post has_many Tags
#[driver_test(id(ID))]
pub async fn nested_belongs_to_required_then_has_many(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[has_many]
        tags: toasty::HasMany<Tag>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Tag {
        #[key]
        #[auto]
        id: ID,

        label: String,

        #[index]
        post_id: ID,

        #[belongs_to(key = post_id, references = id)]
        post: toasty::BelongsTo<Post>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Comment {
        #[key]
        #[auto]
        id: ID,

        body: String,

        #[index]
        post_id: ID,

        #[belongs_to(key = post_id, references = id)]
        post: toasty::BelongsTo<Post>,
    }

    let mut db = test.setup_db(models!(Post, Tag, Comment)).await;

    let post = Post::create()
        .title("Hello")
        .tag(Tag::create().label("rust"))
        .tag(Tag::create().label("orm"))
        .exec(&mut db)
        .await?;

    let comment = Comment::create()
        .body("Nice post")
        .post(&post)
        .exec(&mut db)
        .await?;

    // From comment, preload post's tags
    let comment = Comment::filter_by_id(comment.id)
        .include(Comment::fields().post().tags())
        .get(&mut db)
        .await?;

    assert_eq!("Hello", comment.post.get().title);
    let mut labels: Vec<&str> = comment
        .post
        .get()
        .tags
        .get()
        .iter()
        .map(|t| t.label.as_str())
        .collect();
    labels.sort();
    assert_eq!(labels, vec!["orm", "rust"]);

    Ok(())
}

// ===== BelongsTo<T> (required) -> HasOne<Option<T>> =====
// Todo belongs_to a User, User has_one optional Profile
#[driver_test(id(ID))]
pub async fn nested_belongs_to_required_then_has_one_optional(test: &mut Test) -> Result<()> {
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

    let mut db = test.setup_db(models!(User, Profile, Todo)).await;

    // User with profile
    let user = User::create()
        .name("Alice")
        .profile(Profile::create().bio("developer"))
        .todo(Todo::create().title("Task 1"))
        .exec(&mut db)
        .await?;

    let todo_id = Todo::get_by_user_id(&mut db, user.id).await?.id;

    let todo = Todo::filter_by_id(todo_id)
        .include(Todo::fields().user().profile())
        .get(&mut db)
        .await?;

    assert_eq!("Alice", todo.user.get().name);
    let profile = todo.user.get().profile.get().as_ref().unwrap();
    assert_eq!("developer", profile.bio);

    // User without profile
    let user2 = User::create()
        .name("Bob")
        .todo(Todo::create().title("Task 2"))
        .exec(&mut db)
        .await?;

    let todo2_id = Todo::get_by_user_id(&mut db, user2.id).await?.id;

    let todo2 = Todo::filter_by_id(todo2_id)
        .include(Todo::fields().user().profile())
        .get(&mut db)
        .await?;

    assert_eq!("Bob", todo2.user.get().name);
    assert!(todo2.user.get().profile.get().is_none());

    Ok(())
}

// ===== BelongsTo<T> (required) -> BelongsTo<T> (required) =====
// Step belongs_to a Todo, Todo belongs_to a User (chain of belongs_to going up)
#[driver_test(id(ID))]
pub async fn nested_belongs_to_required_then_belongs_to_required(test: &mut Test) -> Result<()> {
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

    let mut db = test.setup_db(models!(User, Todo, Step)).await;

    let user = User::create()
        .name("Alice")
        .todo(
            Todo::create()
                .title("T1")
                .step(Step::create().description("S1")),
        )
        .exec(&mut db)
        .await?;

    let todo_id = Todo::get_by_user_id(&mut db, user.id).await?.id;
    let step_id = Step::get_by_todo_id(&mut db, todo_id).await?.id;

    // From step, preload todo and then todo's user
    let step = Step::filter_by_id(step_id)
        .include(Step::fields().todo().user())
        .get(&mut db)
        .await?;

    assert_eq!("T1", step.todo.get().title);
    assert_eq!("Alice", step.todo.get().user.get().name);

    Ok(())
}

// ===== BelongsTo<Option<T>> -> HasMany =====
// Task optionally belongs_to a Project, Project has_many Members
#[driver_test(id(ID))]
pub async fn nested_belongs_to_optional_then_has_many(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Project {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        members: toasty::HasMany<Member>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Member {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[index]
        project_id: ID,

        #[belongs_to(key = project_id, references = id)]
        project: toasty::BelongsTo<Project>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Task {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        project_id: Option<ID>,

        #[belongs_to(key = project_id, references = id)]
        project: toasty::BelongsTo<Option<Project>>,
    }

    let mut db = test.setup_db(models!(Project, Member, Task)).await;

    let project = Project::create()
        .name("Proj1")
        .member(Member::create().name("Alice"))
        .member(Member::create().name("Bob"))
        .exec(&mut db)
        .await?;

    // Task with project
    let task = Task::create()
        .title("Linked")
        .project(&project)
        .exec(&mut db)
        .await?;

    let task = Task::filter_by_id(task.id)
        .include(Task::fields().project().members())
        .get(&mut db)
        .await?;

    let proj = task.project.get().as_ref().unwrap();
    assert_eq!("Proj1", proj.name);
    let mut names: Vec<&str> = proj.members.get().iter().map(|m| m.name.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["Alice", "Bob"]);

    // Task without project
    let orphan = Task::create().title("Orphan").exec(&mut db).await?;

    let orphan = Task::filter_by_id(orphan.id)
        .include(Task::fields().project().members())
        .get(&mut db)
        .await?;

    assert!(orphan.project.get().is_none());

    Ok(())
}

// ===== BelongsTo<Option<T>> -> BelongsTo<Option<T>> =====
// Comment optionally belongs_to a Post, Post optionally belongs_to a Category
#[driver_test(id(ID))]
pub async fn nested_belongs_to_optional_then_belongs_to_optional(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Category {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        category_id: Option<ID>,

        #[belongs_to(key = category_id, references = id)]
        category: toasty::BelongsTo<Option<Category>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Comment {
        #[key]
        #[auto]
        id: ID,

        body: String,

        #[index]
        post_id: Option<ID>,

        #[belongs_to(key = post_id, references = id)]
        post: toasty::BelongsTo<Option<Post>>,
    }

    let mut db = test.setup_db(models!(Category, Post, Comment)).await;

    let cat = Category::create().name("Tech").exec(&mut db).await?;
    let post = Post::create()
        .title("Hello")
        .category(&cat)
        .exec(&mut db)
        .await?;

    // Comment -> Post (present) -> Category (present)
    let c1 = Comment::create()
        .body("Nice")
        .post(&post)
        .exec(&mut db)
        .await?;

    let c1 = Comment::filter_by_id(c1.id)
        .include(Comment::fields().post().category())
        .get(&mut db)
        .await?;

    let loaded_post = c1.post.get().as_ref().unwrap();
    assert_eq!("Hello", loaded_post.title);
    let loaded_cat = loaded_post.category.get().as_ref().unwrap();
    assert_eq!("Tech", loaded_cat.name);

    // Post without category
    let post2 = Post::create().title("Uncategorized").exec(&mut db).await?;
    let c2 = Comment::create()
        .body("Hmm")
        .post(&post2)
        .exec(&mut db)
        .await?;

    let c2 = Comment::filter_by_id(c2.id)
        .include(Comment::fields().post().category())
        .get(&mut db)
        .await?;

    let loaded_post2 = c2.post.get().as_ref().unwrap();
    assert_eq!("Uncategorized", loaded_post2.title);
    assert!(loaded_post2.category.get().is_none());

    // Comment without post
    let c3 = Comment::create().body("Orphan").exec(&mut db).await?;

    let c3 = Comment::filter_by_id(c3.id)
        .include(Comment::fields().post().category())
        .get(&mut db)
        .await?;

    assert!(c3.post.get().is_none());

    Ok(())
}

// ===== BelongsTo<T> -> HasOne<T> (required) =====
// Todo belongs_to a User, User has_one required Config
#[driver_test(id(ID))]
pub async fn nested_belongs_to_required_then_has_one_required(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        config: toasty::HasOne<Config>,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Config {
        #[key]
        #[auto]
        id: ID,

        theme: String,

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

    let mut db = test.setup_db(models!(User, Config, Todo)).await;

    let user = User::create()
        .name("Alice")
        .config(Config::create().theme("dark"))
        .todo(Todo::create().title("Task"))
        .exec(&mut db)
        .await?;

    let todo_id = Todo::get_by_user_id(&mut db, user.id).await?.id;

    let todo = Todo::filter_by_id(todo_id)
        .include(Todo::fields().user().config())
        .get(&mut db)
        .await?;

    assert_eq!("Alice", todo.user.get().name);
    assert_eq!("dark", todo.user.get().config.get().theme);

    Ok(())
}

// ===== HasMany -> HasMany (with empty nested collections) =====
// Ensures that when some parents have children and others don't, nested preload
// correctly assigns empty collections rather than panicking.
#[driver_test(id(ID))]
pub async fn nested_has_many_then_has_many_with_empty_leaves(test: &mut Test) {
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

    let mut db = test.setup_db(models!(User, Todo, Step)).await;

    let user = User::create()
        .name("Alice")
        .todo(
            Todo::create()
                .title("With Steps")
                .step(Step::create().description("S1")),
        )
        .todo(Todo::create().title("No Steps")) // empty nested
        .exec(&mut db)
        .await
        .unwrap();

    let user = User::filter_by_id(user.id)
        .include(User::fields().todos().steps())
        .get(&mut db)
        .await
        .unwrap();

    let todos = user.todos.get();
    assert_eq!(2, todos.len());

    let mut total_steps = 0;
    for todo in todos {
        let steps = todo.steps.get();
        if todo.title == "With Steps" {
            assert_eq!(1, steps.len());
            assert_eq!("S1", steps[0].description);
        } else {
            assert_eq!(0, steps.len());
        }
        total_steps += steps.len();
    }
    assert_eq!(1, total_steps);
}

// ===== Issue #691: multiple nested includes sharing a prefix =====
// When several `.include()` calls share a common prefix (e.g. `todos()`), each
// sibling nested include must be preserved — previously the second overwrote
// the first at the shared field slot.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_multi_relation))]
pub async fn sibling_nested_includes_on_shared_prefix(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let category = Category::create().name("Food").exec(&mut db).await?;
    let user = User::create()
        .name("Alice")
        .todo(Todo::create().title("T1").category(&category))
        .todo(Todo::create().title("T2").category(&category))
        .exec(&mut db)
        .await?;

    // Two sibling nested includes under the `todos()` prefix. Both must be
    // preloaded — neither should be silently clobbered by the other.
    let loaded = User::filter_by_id(user.id)
        .include(User::fields().todos().user())
        .include(User::fields().todos().category())
        .get(&mut db)
        .await?;

    let todos = loaded.todos.get();
    assert_eq!(2, todos.len());
    for todo in todos {
        assert_eq!("Alice", todo.user.get().name);
        assert_eq!("Food", todo.category.get().name);
    }

    Ok(())
}

// Mirrors the exact pattern from issue #691: a bare top-level include plus
// two sibling nested includes sharing that same top-level prefix. All three
// paths must be honored.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_multi_relation))]
pub async fn bare_and_nested_includes_on_shared_prefix(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let category = Category::create().name("Food").exec(&mut db).await?;
    let user = User::create()
        .name("Alice")
        .todo(Todo::create().title("T1").category(&category))
        .exec(&mut db)
        .await?;

    let loaded = User::filter_by_id(user.id)
        .include(User::fields().todos()) // bare
        .include(User::fields().todos().user()) // sibling 1
        .include(User::fields().todos().category()) // sibling 2
        .get(&mut db)
        .await?;

    let todos = loaded.todos.get();
    assert_eq!(1, todos.len());
    assert_eq!("Alice", todos[0].user.get().name);
    assert_eq!("Food", todos[0].category.get().name);

    Ok(())
}
