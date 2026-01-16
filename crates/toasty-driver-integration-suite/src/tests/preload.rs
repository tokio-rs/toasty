use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn basic_has_many_and_belongs_to_preload(test: &mut Test) {
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
        .await
        .unwrap();

    // Find the user, include TODOs
    let user = User::filter_by_id(user.id)
        .include(User::FIELDS.todos())
        .get(&db)
        .await
        .unwrap();

    // This will panic
    assert_eq!(3, user.todos.get().len());

    let id = user.todos.get()[0].id;

    let todo = Todo::filter_by_id(id)
        .include(Todo::FIELDS.user())
        .get(&db)
        .await
        .unwrap();

    assert_eq!(user.id, todo.user.get().id);
    assert_eq!(user.id, todo.user_id);
}

#[driver_test(id(ID))]
pub async fn multiple_includes_same_model(test: &mut Test) {
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
    let user = User::create().name("Test User").exec(&db).await.unwrap();

    // Create posts associated with the user
    Post::create()
        .title("Post 1")
        .user(&user)
        .exec(&db)
        .await
        .unwrap();

    Post::create()
        .title("Post 2")
        .user(&user)
        .exec(&db)
        .await
        .unwrap();

    // Create comments associated with the user
    Comment::create()
        .text("Comment 1")
        .user(&user)
        .exec(&db)
        .await
        .unwrap();

    Comment::create()
        .text("Comment 2")
        .user(&user)
        .exec(&db)
        .await
        .unwrap();

    Comment::create()
        .text("Comment 3")
        .user(&user)
        .exec(&db)
        .await
        .unwrap();

    // Test individual includes work (baseline)
    let user_with_posts = User::filter_by_id(user.id)
        .include(User::FIELDS.posts())
        .get(&db)
        .await
        .unwrap();
    assert_eq!(2, user_with_posts.posts.get().len());

    let user_with_comments = User::filter_by_id(user.id)
        .include(User::FIELDS.comments())
        .get(&db)
        .await
        .unwrap();
    assert_eq!(3, user_with_comments.comments.get().len());

    // Test multiple includes in one query
    let loaded_user = User::filter_by_id(user.id)
        .include(User::FIELDS.posts()) // First include
        .include(User::FIELDS.comments()) // Second include
        .get(&db)
        .await
        .unwrap();

    assert_eq!(2, loaded_user.posts.get().len());
    assert_eq!(3, loaded_user.comments.get().len());
}

#[driver_test(id(ID))]
pub async fn basic_has_one_and_belongs_to_preload(test: &mut Test) {
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
        .await
        .unwrap();

    // Find the user, include profile
    let user = User::filter_by_id(user.id)
        .include(User::FIELDS.profile())
        .get(&db)
        .await
        .unwrap();

    // Verify the profile is preloaded
    let profile = user.profile.get().as_ref().unwrap();
    assert_eq!("A person", profile.bio);
    assert_eq!(user.id, *profile.user_id.as_ref().unwrap());

    let profile_id = profile.id;

    // Test the reciprocal belongs_to preload
    let profile = Profile::filter_by_id(profile_id)
        .include(Profile::FIELDS.user())
        .get(&db)
        .await
        .unwrap();

    assert_eq!(user.id, profile.user.get().as_ref().unwrap().id);
    assert_eq!("John Doe", profile.user.get().as_ref().unwrap().name);
}

#[driver_test(id(ID))]
pub async fn multiple_includes_with_has_one(test: &mut Test) {
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
        .await
        .unwrap();

    // Test individual includes work (baseline)
    let user_with_profile = User::filter_by_id(user.id)
        .include(User::FIELDS.profile())
        .get(&db)
        .await
        .unwrap();
    assert!(user_with_profile.profile.get().is_some());
    assert_eq!(
        "Software engineer",
        user_with_profile.profile.get().as_ref().unwrap().bio
    );

    let user_with_settings = User::filter_by_id(user.id)
        .include(User::FIELDS.settings())
        .get(&db)
        .await
        .unwrap();
    assert!(user_with_settings.settings.get().is_some());
    assert_eq!(
        "dark",
        user_with_settings.settings.get().as_ref().unwrap().theme
    );

    // Test multiple includes in one query
    let loaded_user = User::filter_by_id(user.id)
        .include(User::FIELDS.profile()) // First include
        .include(User::FIELDS.settings()) // Second include
        .get(&db)
        .await
        .unwrap();

    assert!(loaded_user.profile.get().is_some());
    assert_eq!(
        "Software engineer",
        loaded_user.profile.get().as_ref().unwrap().bio
    );
    assert!(loaded_user.settings.get().is_some());
    assert_eq!("dark", loaded_user.settings.get().as_ref().unwrap().theme);
}

#[driver_test(id(ID))]
pub async fn combined_has_many_and_has_one_preload(test: &mut Test) {
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
        .await
        .unwrap();

    // Test combined has_one and has_many preload in a single query
    let loaded_user = User::filter_by_id(user.id)
        .include(User::FIELDS.profile()) // has_one include
        .include(User::FIELDS.todos()) // has_many include
        .get(&db)
        .await
        .unwrap();

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
}

#[driver_test(id(ID))]
pub async fn preload_on_empty_table(test: &mut Test) {
    if !test.capability().sql {
        return;
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
        .include(User::FIELDS.todos())
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(0, users.len());
}

#[driver_test(id(ID))]
pub async fn preload_on_empty_query(test: &mut Test) {
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
        .include(User::FIELDS.todos())
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(0, users.len());
}
