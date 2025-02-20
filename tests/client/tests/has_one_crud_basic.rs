use tests_client::*;

async fn crud_has_one_bi_direction_optional(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            name: String,

            profile: Option<Profile>,
        }

        model Profile {
            #[key]
            #[auto]
            id: Id,

            #[unique]
            user_id: Option<Id<User>>,

            #[relation(key = user_id, references = id)]
            user: Option<User>,

            bio: String,
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user without a profile
    let user = db::User::create().name("Jane Doe").exec(&db).await.unwrap();

    // No profile
    assert_none!(user.profile().get(&db).await.unwrap());

    // Create a profile for the user
    let profile = user
        .profile()
        .create()
        .bio("a person")
        .exec(&db)
        .await
        .unwrap();

    assert_ne!(user.id.to_string(), profile.id.to_string());

    // Load the profile
    let profile_reload = user.profile().get(&db).await.unwrap().unwrap();
    assert_eq!(profile.id, profile_reload.id);

    // Load the user via the profile
    let user_reload = profile.user().get(&db).await.unwrap().unwrap();
    assert_eq!(user.id, user_reload.id);

    // Create a new user with a profile
    let mut user = db::User::create()
        .name("Tim Apple")
        .profile(db::Profile::create().bio("an apple a day"))
        .exec(&db)
        .await
        .unwrap();

    let profile = user.profile().get(&db).await.unwrap().unwrap();
    assert_eq!(profile.bio, "an apple a day");

    // The new profile is associated with the user
    assert_eq!(user.id, profile.user().get(&db).await.unwrap().unwrap().id);

    // Update a user, creating a new profile.
    user.update()
        .profile(db::Profile::create().bio("keeps the doctor away"))
        .exec(&db)
        .await
        .unwrap();

    // The user's profile is updated
    let profile = user.profile().get(&db).await.unwrap().unwrap();
    assert_eq!(profile.bio, "keeps the doctor away");
    assert_eq!(user.id, profile.user().get(&db).await.unwrap().unwrap().id);

    // Unset the profile via an update. This will nullify user on the profile.
    let mut update = user.update();
    update.unset_profile();
    update.exec(&db).await.unwrap();

    // The profile is none
    assert!(user.profile().get(&db).await.unwrap().is_none());

    let profile_reloaded = db::Profile::filter_by_id(&profile.id)
        .get(&db)
        .await
        .unwrap();
    assert_none!(profile_reloaded.user_id);

    user.update()
        .profile(&profile_reloaded)
        .exec(&db)
        .await
        .unwrap();

    let profile_reloaded = db::Profile::get_by_id(&db, &profile.id).await.unwrap();
    assert_eq!(&user.id, profile_reloaded.user_id.as_ref().unwrap());

    // Deleting the profile will nullify the profile field for the user
    profile_reloaded.delete(&db).await.unwrap();

    let mut user_reloaded = db::User::get_by_id(&db, &user.id).await.unwrap();
    assert_none!(user_reloaded.profile().get(&db).await.unwrap());

    // Create a new profile for the user
    user_reloaded
        .update()
        .profile(db::Profile::create().bio("hello"))
        .exec(&db)
        .await
        .unwrap();

    let profile_id = user_reloaded.profile().get(&db).await.unwrap().unwrap().id;

    // Delete the user
    user_reloaded.delete(&db).await.unwrap();

    let profile_reloaded = db::Profile::get_by_id(&db, &profile_id).await.unwrap();
    assert_none!(profile_reloaded.user_id);
}

async fn crud_has_one_required_belongs_to_optional(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            name: String,

            profile: Profile,
        }

        model Profile {
            #[key]
            #[auto]
            id: Id,

            #[unique]
            user_id: Option<Id<User>>,

            #[relation(key = user_id, references = id)]
            user: Option<User>,

            bio: String,
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    // Create a new user with a profile
    let user = db::User::create()
        .name("Tim Apple")
        .profile(db::Profile::create().bio("an apple a day"))
        .exec(&db)
        .await
        .unwrap();

    let profile = user.profile().get(&db).await.unwrap();
    assert_eq!(profile.bio, "an apple a day");

    // The new profile is associated with the user
    assert_eq!(user.id, profile.user().get(&db).await.unwrap().unwrap().id);

    // Deleting the user leaves the profile in place.
    user.delete(&db).await.unwrap();
    let profile_reloaded = db::Profile::get_by_id(&db, &profile.id).await.unwrap();
    assert_none!(profile_reloaded.user_id);

    // Try creating a user **without** a user: error
    assert_err!(db::User::create().name("Nop Rofile").exec(&db).await);
}

async fn update_belongs_to_with_required_has_one_pair(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            name: String,

            profile: Profile,
        }

        model Profile {
            #[key]
            #[auto]
            id: Id,

            #[unique]
            user_id: Option<Id<User>>,

            #[relation(key = user_id, references = id)]
            user: Option<User>,

            bio: String,
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user with a profile
    let u1 = db::User::create()
        .name("Tim Apple")
        .profile(db::Profile::create().bio("an apple a day"))
        .exec(&db)
        .await
        .unwrap();

    let mut p1 = u1.profile().get(&db).await.unwrap();
    assert_eq!(p1.bio, "an apple a day");

    assert_ne!(u1.id.to_string(), p1.id.to_string());

    // Associate the profile with a new user by value
    let u2 = db::User::create()
        .name("Johnny Appleseed")
        .profile(db::Profile::create().bio("I plant trees"))
        .exec(&db)
        .await
        .unwrap();

    let p2 = u2.profile().get(&db).await.unwrap();
    assert_eq!(p2.bio, "I plant trees");

    // Associate the original profile w/ the new user by value
    p1.update().user(&u2).exec(&db).await.unwrap();

    println!("--------------");
    // assert_eq!(u2.id, p1.user().find(&db).await.unwrap().unwrap().id);
    // u1 is deleted
    assert_err!(db::User::get_by_id(&db, &u1.id).await);
    // p2 ID is null
    let p2_reloaded = db::Profile::get_by_id(&db, &p2.id).await.unwrap();
    assert_none!(p2_reloaded.user_id);

    /*
    // Associate the profile with a new user by statement
    let u1 = db::User::create()
        .name("Tim Apple")
        .profile(db::Profile::create().bio("an apple a day"))
        .exec(&db)
        .await
        .unwrap();

    let mut p1 = u1.profile().get(&db).await.unwrap();
    assert_eq!(p1.bio, "an apple a day");

    /*
    // Associate the profile with a new user by value
    let u2 = db::User::create()
        .name("Johnny Appleseed")
        .profile(db::Profile::create().bio("I plant trees"))
        .exec(&db)
        .await
        .unwrap();

    let p2 = u2.profile().get(&db).await.unwrap();
    assert_eq!(p2.bio, "I plant trees");
    */

    // Associate the original profile w/ the new user by value
    p1.update()
        .user(db::User::create().name("Johnny Appleseed"))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(
        u2.id,
        p1.user
            .as_ref()
            .unwrap()
            .get(&db)
            .await
            .unwrap()
            .unwrap()
            .id
    );
    // u1 is deleted
    assert_err!(db::User::find_by_id(&u1.id).get(&db).await);
    */
}

async fn crud_has_one_optional_belongs_to_required(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            name: String,

            profile: Option<Profile>,
        }

        model Profile {
            #[key]
            #[auto]
            id: Id,

            #[unique]
            user_id: Id<User>,

            #[relation(key = user_id, references = id)]
            user: User,

            bio: String,
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    // Create a new user with a profile
    let user = db::User::create()
        .name("Tim Apple")
        .profile(db::Profile::create().bio("an apple a day"))
        .exec(&db)
        .await
        .unwrap();

    let profile = user.profile().get(&db).await.unwrap().unwrap();
    assert_eq!(profile.bio, "an apple a day");

    // The new profile is associated with the user
    assert_eq!(user.id, profile.user().get(&db).await.unwrap().id);

    // Deleting the user also deletes the profile
    user.delete(&db).await.unwrap();
    assert_err!(db::Profile::get_by_id(&db, &profile.id).await);
}

async fn has_one_must_specify_relation_on_one_side(_s: impl Setup) {
    toasty_core::schema::from_str(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            profile: Option<Profile>,
        }

        model Profile {
            #[key]
            #[auto]
            id: Id,

            user: User,
        }
        ",
    )
    .unwrap();
}

async fn has_one_must_specify_be_uniquely_indexed(_s: impl Setup) {
    toasty_core::schema::from_str(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            profile: Option<Profile>,
        }

        model Profile {
            #[key]
            #[auto]
            id: Id,

            [index]
            user_id: Id<User>,

            #[relation(key = user_id, references = id)]
            user: User,
        }
        ",
    )
    .unwrap();
}

async fn set_has_one_by_value_in_update_query(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            profile: Option<Profile>,
        }

        model Profile {
            #[key]
            #[auto]
            id: Id,

            #[unique]
            user_id: Option<Id<User>>,

            #[relation(key = user_id, references = id)]
            user: Option<User>,
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    let user = db::User::create().exec(&db).await.unwrap();
    let profile = db::Profile::create().exec(&db).await.unwrap();

    db::User::filter_by_id(&user.id)
        .update()
        .profile(&profile)
        .exec(&db)
        .await
        .unwrap();

    let profile_reload = user.profile().get(&db).await.unwrap().unwrap();
    assert_eq!(profile_reload.id, profile.id);

    assert_eq!(profile_reload.user_id.as_ref().unwrap(), &user.id);
}

async fn unset_has_one_in_batch_update(_s: impl Setup) {}

async fn unset_has_one_with_required_pair_in_pk_query_update(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            profile: Option<Profile>,
        }

        model Profile {
            #[key]
            #[auto]
            id: Id,

            #[unique]
            user_id: Id<User>,

            #[relation(key = user_id, references = id)]
            user: User,
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    let user = db::User::create()
        .profile(db::Profile::create())
        .exec(&db)
        .await
        .unwrap();
    let profile = user.profile().get(&db).await.unwrap().unwrap();

    println!("=======> here");
    let mut update = db::User::filter_by_id(&user.id).update();
    update.unset_profile();
    update.exec(&db).await.unwrap();

    // Profile is deleted
    assert_err!(db::Profile::get_by_id(&db, &profile.id).await);
}

async fn unset_has_one_with_required_pair_in_non_pk_query_update(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            #[unique]
            email: String,

            profile: Option<Profile>,
        }

        model Profile {
            #[key]
            #[auto]
            id: Id,

            #[unique]
            user_id: Id<User>,

            #[relation(key = user_id, references = id)]
            user: User,
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    let user = db::User::create()
        .email("foo@example.com")
        .profile(db::Profile::create())
        .exec(&db)
        .await
        .unwrap();
    let profile = user.profile().get(&db).await.unwrap().unwrap();

    println!("=======> here");
    let mut update = db::User::filter_by_email(&user.email).update();
    update.unset_profile();
    update.exec(&db).await.unwrap();

    // Profile is deleted
    assert_err!(db::Profile::get_by_id(&db, &profile.id).await);
}

async fn associate_has_one_by_val_on_insert(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            name: String,

            profile: Profile,
        }

        model Profile {
            #[key]
            #[auto]
            id: Id,

            #[unique]
            user_id: Option<Id<User>>,

            #[relation(key = user_id, references = id)]
            user: Option<User>,

            bio: String,
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    // Create a profile
    let profile = db::Profile::create()
        .bio("hello world")
        .exec(&db)
        .await
        .unwrap();

    // Create a user and associate the profile with it, by value
    let u1 = db::User::create()
        .name("User 1")
        .profile(&profile)
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(profile.id, u1.profile().get(&db).await.unwrap().id);
}

async fn associate_has_one_by_val_on_update_query_with_filter(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            name: String,

            profile: Option<Profile>,
        }

        model Profile {
            #[key]
            #[auto]
            id: Id,

            #[unique]
            user_id: Option<Id<User>>,

            #[relation(key = user_id, references = id)]
            user: Option<User>,

            bio: String,
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    let u1 = db::User::create().name("user 1").exec(&db).await.unwrap();
    let p1 = db::Profile::create()
        .bio("hello world")
        .exec(&db)
        .await
        .unwrap();

    db::User::filter_by_id(&u1.id)
        .update()
        .profile(&p1)
        .exec(&db)
        .await
        .unwrap();

    let u1_reloaded = db::User::get_by_id(&db, &u1.id).await.unwrap();
    assert_eq!(
        p1.id,
        u1_reloaded.profile().get(&db).await.unwrap().unwrap().id
    );

    // Unset
    let mut update = db::User::filter_by_id(&u1.id).update();
    update.unset_profile();
    update.exec(&db).await.unwrap();

    // Getting this to work will require a big chunk of work in the planner.
    db::User::filter_by_id(&u1.id)
        .filter(db::User::NAME.eq("anon"))
        .update()
        .profile(&p1)
        .exec(&db)
        .await
        .unwrap();
}

tests!(
    crud_has_one_bi_direction_optional,
    // TODO: this should not actually panic
    #[should_panic(expected = "Insert missing non-nullable field; model=User; field=profile")]
    crud_has_one_required_belongs_to_optional,
    update_belongs_to_with_required_has_one_pair,
    crud_has_one_optional_belongs_to_required,
    #[should_panic(expected = "no relation pair for User::profile")]
    has_one_must_specify_relation_on_one_side,
    #[ignore]
    #[should_panic(expected = "lol")]
    has_one_must_specify_be_uniquely_indexed,
    set_has_one_by_value_in_update_query,
    #[ignore]
    unset_has_one_in_batch_update,
    unset_has_one_with_required_pair_in_pk_query_update,
    unset_has_one_with_required_pair_in_non_pk_query_update,
    associate_has_one_by_val_on_insert,
    #[ignore]
    associate_has_one_by_val_on_update_query_with_filter,
);
