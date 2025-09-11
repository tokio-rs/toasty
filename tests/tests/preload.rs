use tests::{models, tests, DbTest};
use toasty::stmt::Id;

async fn basic_has_many_and_belongs_to_preload(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: Id<Self>,

        #[index]
        #[allow(dead_code)]
        user_id: Id<User>,

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
    let user = User::filter_by_id(&user.id)
        .include(User::FIELDS.todos())
        .get(&db)
        .await
        .unwrap();

    // This will panic
    assert_eq!(3, user.todos.get().len());

    let id = user.todos.get()[0].id.clone();

    let todo = Todo::filter_by_id(&id)
        .include(Todo::FIELDS.user())
        .get(&db)
        .await
        .unwrap();

    assert_eq!(user.id, todo.user.get().id);
    assert_eq!(user.id, todo.user_id);
}

async fn multiple_includes_same_model(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

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
        id: Id<Self>,

        #[allow(dead_code)]
        title: String,

        #[index]
        #[allow(dead_code)]
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    #[derive(Debug, toasty::Model)]
    struct Comment {
        #[key]
        #[auto]
        id: Id<Self>,

        #[allow(dead_code)]
        text: String,

        #[index]
        #[allow(dead_code)]
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let db = test.setup_db(models!(User, Post, Comment)).await;

    // NOTE:
    // 1. Temporarily change these values to (500, 100, 100)
    // 2. Run with --nocapture to see timing
    // 3. Revert to old algorithm and compare
    let num_users = 10;
    let posts_per_user = 5;
    let comments_per_user = 5;

    println!(
        "Setting up benchmark data: {} users with {} posts and {} comments each",
        num_users, posts_per_user, comments_per_user
    );

    let mut user_ids = vec![];

    for i in 0..num_users {
        let user = User::create()
            .name(&format!("User {}", i))
            .exec(&db)
            .await
            .unwrap();

        user_ids.push(user.id.clone());

        for j in 0..posts_per_user {
            Post::create()
                .title(&format!("Post {} for User {}", j, i))
                .user(&user)
                .exec(&db)
                .await
                .unwrap();
        }

        for j in 0..comments_per_user {
            Comment::create()
                .text(&format!("Comment {} for User {}", j, i))
                .user(&user)
                .exec(&db)
                .await
                .unwrap();
        }
    }

    println!(
        "Created {} posts and {} comments total",
        num_users * posts_per_user,
        num_users * comments_per_user
    );
    println!("---");

    let start = std::time::Instant::now();

    let users_with_associations: Vec<User> = User::all()
        .include(User::FIELDS.posts())
        .include(User::FIELDS.comments())
        .collect(&db)
        .await
        .unwrap();

    let duration = start.elapsed();

    println!("Performance Results:");
    println!(
        "  Loading {} users with all associations: {:?}",
        users_with_associations.len(),
        duration
    );
    println!(
        "  Total associations processed: {}",
        num_users * (posts_per_user + comments_per_user)
    );
    println!(
        "  Average time per association: {:?}",
        duration / (num_users * (posts_per_user + comments_per_user)) as u32
    );

    assert_eq!(num_users, users_with_associations.len());
    for user in &users_with_associations {
        assert_eq!(posts_per_user, user.posts.get().len());
        assert_eq!(comments_per_user, user.comments.get().len());
    }

    let start_single = std::time::Instant::now();
    let single_user = User::filter_by_id(&user_ids[0])
        .include(User::FIELDS.posts())
        .include(User::FIELDS.comments())
        .get(&db)
        .await
        .unwrap();
    let duration_single = start_single.elapsed();

    println!(
        "  Loading single user with associations: {:?}",
        duration_single
    );

    assert_eq!(posts_per_user, single_user.posts.get().len());
    assert_eq!(comments_per_user, single_user.comments.get().len());

    let user = User::create().name("Test User").exec(&db).await.unwrap();

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

    let user_with_posts = User::filter_by_id(&user.id)
        .include(User::FIELDS.posts())
        .get(&db)
        .await
        .unwrap();
    assert_eq!(2, user_with_posts.posts.get().len());

    let user_with_comments = User::filter_by_id(&user.id)
        .include(User::FIELDS.comments())
        .get(&db)
        .await
        .unwrap();
    assert_eq!(3, user_with_comments.comments.get().len());

    let loaded_user = User::filter_by_id(&user.id)
        .include(User::FIELDS.posts())
        .include(User::FIELDS.comments())
        .get(&db)
        .await
        .unwrap();

    assert_eq!(2, loaded_user.posts.get().len());
    assert_eq!(3, loaded_user.comments.get().len());
}

async fn basic_has_one_and_belongs_to_preload(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        name: String,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: Id<Self>,

        bio: String,

        #[unique]
        user_id: Option<Id<User>>,

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
    let user = User::filter_by_id(&user.id)
        .include(User::FIELDS.profile())
        .get(&db)
        .await
        .unwrap();

    // Verify the profile is preloaded
    let profile = user.profile.get().as_ref().unwrap();
    assert_eq!("A person", profile.bio);
    assert_eq!(user.id, *profile.user_id.as_ref().unwrap());

    let profile_id = profile.id.clone();

    // Test the reciprocal belongs_to preload
    let profile = Profile::filter_by_id(&profile_id)
        .include(Profile::FIELDS.user())
        .get(&db)
        .await
        .unwrap();

    assert_eq!(user.id, profile.user.get().as_ref().unwrap().id);
    assert_eq!("John Doe", profile.user.get().as_ref().unwrap().name);
}

async fn multiple_includes_with_has_one(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

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
        id: Id<Self>,

        bio: String,

        #[unique]
        user_id: Option<Id<User>>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Settings {
        #[key]
        #[auto]
        id: Id<Self>,

        theme: String,

        #[unique]
        user_id: Option<Id<User>>,

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
    let user_with_profile = User::filter_by_id(&user.id)
        .include(User::FIELDS.profile())
        .get(&db)
        .await
        .unwrap();
    assert!(user_with_profile.profile.get().is_some());
    assert_eq!(
        "Software engineer",
        user_with_profile.profile.get().as_ref().unwrap().bio
    );

    let user_with_settings = User::filter_by_id(&user.id)
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
    let loaded_user = User::filter_by_id(&user.id)
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

async fn combined_has_many_and_has_one_preload(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

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
        id: Id<Self>,

        bio: String,

        #[unique]
        user_id: Option<Id<User>>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: Id<Self>,

        title: String,

        #[index]
        user_id: Id<User>,

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
    let loaded_user = User::filter_by_id(&user.id)
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

tests!(
    basic_has_many_and_belongs_to_preload,
    multiple_includes_same_model,
    basic_has_one_and_belongs_to_preload,
    multiple_includes_with_has_one,
    combined_has_many_and_has_one_preload
);
