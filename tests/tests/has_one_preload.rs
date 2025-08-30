use tests::{models, tests, DbTest};
use toasty::stmt::Id;

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

        #[unique]
        user_id: Option<Id<User>>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        bio: String,
    }

    let db = test.setup_db(models!(User, Profile)).await;

    // Create a user with a profile
    let user = User::create()
        .name("Jane Doe")
        .profile(Profile::create().bio("Software developer"))
        .exec(&db)
        .await
        .unwrap();

    // Find the user, include profile
    let user_with_profile = User::filter_by_id(&user.id)
        .include(User::FIELDS.profile())
        .get(&db)
        .await
        .unwrap();

    // Profile should be preloaded
    let profile = user_with_profile.profile.get();
    assert!(profile.is_some());
    let profile = profile.as_ref().unwrap();
    assert_eq!(profile.bio, "Software developer");
    assert_eq!(profile.user_id.as_ref().unwrap(), &user.id);

    // Test the reciprocal belongs_to preload
    let profile_with_user = Profile::filter_by_id(&profile.id)
        .include(Profile::FIELDS.user())
        .get(&db)
        .await
        .unwrap();

    // User should be preloaded
    let preloaded_user = profile_with_user.user.get();
    assert!(preloaded_user.is_some());
    let preloaded_user = preloaded_user.as_ref().unwrap();
    assert_eq!(preloaded_user.id, user.id);
    assert_eq!(preloaded_user.name, "Jane Doe");
}

async fn multiple_includes_same_model(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
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
    struct Profile {
        #[key]
        #[auto]
        id: Id<Self>,

        #[unique]
        user_id: Option<Id<User>>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        bio: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Settings {
        #[key]
        #[auto]
        id: Id<Self>,

        #[unique]
        user_id: Option<Id<User>>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        theme: String,
        notifications: i32,
    }

    let db = test.setup_db(models!(User, Profile, Settings)).await;

    // Create a user
    let user = User::create().name("Test User").exec(&db).await.unwrap();

    // Create profile associated with the user
    let _profile = Profile::create()
        .bio("Test bio")
        .user(&user)
        .exec(&db)
        .await
        .unwrap();

    // Create settings associated with the user
    let _settings = Settings::create()
        .theme("dark")
        .notifications(1)
        .user(&user)
        .exec(&db)
        .await
        .unwrap();

    // Test individual includes work (baseline)
    let user_with_profile = User::filter_by_id(&user.id)
        .include(User::FIELDS.profile())
        .get(&db)
        .await
        .unwrap();

    let loaded_profile = user_with_profile.profile.get();
    assert!(loaded_profile.is_some());
    assert_eq!(loaded_profile.as_ref().unwrap().bio, "Test bio");

    let user_with_settings = User::filter_by_id(&user.id)
        .include(User::FIELDS.settings())
        .get(&db)
        .await
        .unwrap();

    let loaded_settings = user_with_settings.settings.get();
    assert!(loaded_settings.is_some());
    let loaded_settings = loaded_settings.as_ref().unwrap();
    assert_eq!(loaded_settings.theme, "dark");
    assert_eq!(loaded_settings.notifications, 1);

    // Test multiple includes in one query
    let loaded_user = User::filter_by_id(&user.id)
        .include(User::FIELDS.profile()) // First include
        .include(User::FIELDS.settings()) // Second include
        .get(&db)
        .await
        .unwrap();

    // Both associations should be preloaded
    let loaded_profile = loaded_user.profile.get();
    assert!(loaded_profile.is_some());
    assert_eq!(loaded_profile.as_ref().unwrap().bio, "Test bio");

    let loaded_settings = loaded_user.settings.get();
    assert!(loaded_settings.is_some());
    let loaded_settings = loaded_settings.as_ref().unwrap();
    assert_eq!(loaded_settings.theme, "dark");
    assert_eq!(loaded_settings.notifications, 1);
}

tests!(
    basic_has_one_and_belongs_to_preload,
    multiple_includes_same_model
);
