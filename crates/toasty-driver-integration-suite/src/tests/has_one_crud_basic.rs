use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn crud_has_one_bi_direction_optional(test: &mut Test) {
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

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        bio: String,
    }

    let db = test.setup_db(models!(User, Profile)).await;

    // Create a user without a profile
    let user = User::create().name("Jane Doe").exec(&db).await.unwrap();

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

    // Load the profile
    let profile_reload = user.profile().get(&db).await.unwrap().unwrap();
    assert_eq!(profile.id, profile_reload.id);

    // Load the user via the profile
    let user_reload = profile.user().get(&db).await.unwrap().unwrap();
    assert_eq!(user.id, user_reload.id);

    // Create a new user with a profile
    let mut user = User::create()
        .name("Tim Apple")
        .profile(Profile::create().bio("an apple a day"))
        .exec(&db)
        .await
        .unwrap();

    let profile = user.profile().get(&db).await.unwrap().unwrap();
    assert_eq!(profile.bio, "an apple a day");

    // The new profile is associated with the user
    assert_eq!(user.id, profile.user().get(&db).await.unwrap().unwrap().id);

    // Update a user, creating a new profile.
    user.update()
        .profile(Profile::create().bio("keeps the doctor away"))
        .exec(&db)
        .await
        .unwrap();

    // The user's profile is updated
    let profile = user.profile().get(&db).await.unwrap().unwrap();
    assert_eq!(profile.bio, "keeps the doctor away");
    assert_eq!(user.id, profile.user().get(&db).await.unwrap().unwrap().id);

    // Unset the profile via an update. This will nullify user on the profile.
    user.update().profile(None).exec(&db).await.unwrap();

    // The profile is none
    assert!(user.profile().get(&db).await.unwrap().is_none());

    let profile_reloaded = Profile::filter_by_id(profile.id).get(&db).await.unwrap();
    assert_none!(profile_reloaded.user_id);

    user.update()
        .profile(&profile_reloaded)
        .exec(&db)
        .await
        .unwrap();

    let profile_reloaded = Profile::get_by_id(&db, &profile.id).await.unwrap();
    assert_eq!(&user.id, profile_reloaded.user_id.as_ref().unwrap());

    // Deleting the profile will nullify the profile field for the user
    profile_reloaded.delete(&db).await.unwrap();

    let mut user_reloaded = User::get_by_id(&db, &user.id).await.unwrap();
    assert_none!(user_reloaded.profile().get(&db).await.unwrap());

    // Create a new profile for the user
    user_reloaded
        .update()
        .profile(Profile::create().bio("hello"))
        .exec(&db)
        .await
        .unwrap();

    let profile_id = user_reloaded.profile().get(&db).await.unwrap().unwrap().id;

    // Delete the user
    user_reloaded.delete(&db).await.unwrap();

    let profile_reloaded = Profile::get_by_id(&db, &profile_id).await.unwrap();
    assert_none!(profile_reloaded.user_id);
}

#[driver_test(id(ID))]
#[should_panic]
pub async fn crud_has_one_required_belongs_to_optional(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_one]
        profile: toasty::HasOne<Profile>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        bio: String,
    }

    let db = test.setup_db(models!(User, Profile)).await;

    // Create a new user with a profile
    let user = User::create()
        .profile(Profile::create().bio("an apple a day"))
        .exec(&db)
        .await
        .unwrap();

    let profile = user.profile().get(&db).await.unwrap();
    assert_eq!(profile.bio, "an apple a day");

    // The new profile is associated with the user
    assert_eq!(user.id, profile.user().get(&db).await.unwrap().unwrap().id);

    // Deleting the user leaves the profile in place.
    user.delete(&db).await.unwrap();
    let profile_reloaded = Profile::get_by_id(&db, &profile.id).await.unwrap();
    assert_none!(profile_reloaded.user_id);

    // Try creating a user **without** a user: error
    assert_err!(User::create().exec(&db).await);
}

#[driver_test(id(ID))]
pub async fn update_belongs_to_with_required_has_one_pair(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_one]
        profile: toasty::HasOne<Profile>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        bio: String,
    }

    let db = test.setup_db(models!(User, Profile)).await;

    // Create a user with a profile
    let u1 = User::create()
        .profile(Profile::create().bio("an apple a day"))
        .exec(&db)
        .await
        .unwrap();

    let mut p1 = u1.profile().get(&db).await.unwrap();
    assert_eq!(p1.bio, "an apple a day");

    // Associate the profile with a new user by value
    let u2 = User::create()
        .profile(Profile::create().bio("I plant trees"))
        .exec(&db)
        .await
        .unwrap();

    let p2 = u2.profile().get(&db).await.unwrap();
    assert_eq!(p2.bio, "I plant trees");

    // Associate the original profile w/ the new user by value
    p1.update().user(&u2).exec(&db).await.unwrap();

    // assert_eq!(u2.id, p1.user().find(&db).await.unwrap().unwrap().id);
    // u1 is deleted
    assert_err!(User::get_by_id(&db, &u1.id).await);
    // p2 ID is null
    let p2_reloaded = Profile::get_by_id(&db, &p2.id).await.unwrap();
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

#[driver_test(id(ID))]
pub async fn crud_has_one_optional_belongs_to_required(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        bio: String,
    }

    let db = test.setup_db(models!(User, Profile)).await;

    // Create a new user with a profile
    let user = User::create()
        .profile(Profile::create().bio("an apple a day"))
        .exec(&db)
        .await
        .unwrap();

    let profile = user.profile().get(&db).await.unwrap().unwrap();
    assert_eq!(profile.bio, "an apple a day");

    // The new profile is associated with the user
    assert_eq!(user.id, profile.user().get(&db).await.unwrap().id);

    // Deleting the user also deletes the profile
    user.delete(&db).await.unwrap();
    assert_err!(Profile::get_by_id(&db, &profile.id).await);
}

// TODO: implement this for proc macros
/*
#[driver_test(id(ID))]
pub async fn has_one_must_specify_be_uniquely_indexed(_test: &mut Test) {
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
*/

#[driver_test(id(ID))]
pub async fn set_has_one_by_value_in_update_query(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    let db = test.setup_db(models!(User, Profile)).await;

    let user = User::create().exec(&db).await.unwrap();
    let profile = Profile::create().exec(&db).await.unwrap();

    User::filter_by_id(user.id)
        .update()
        .profile(&profile)
        .exec(&db)
        .await
        .unwrap();

    let profile_reload = user.profile().get(&db).await.unwrap().unwrap();
    assert_eq!(profile_reload.id, profile.id);

    assert_eq!(profile_reload.user_id.as_ref().unwrap(), &user.id);
}

#[driver_test(id(ID))]
#[ignore]
pub async fn unset_has_one_in_batch_update(_test: &mut Test) {}

#[driver_test(id(ID))]
pub async fn unset_has_one_with_required_pair_in_pk_query_update(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let db = test.setup_db(models!(User, Profile)).await;

    let user = User::create()
        .profile(Profile::create())
        .exec(&db)
        .await
        .unwrap();
    let profile = user.profile().get(&db).await.unwrap().unwrap();

    assert_eq!(user.id, profile.user_id);

    User::filter_by_id(user.id)
        .update()
        .profile(None)
        .exec(&db)
        .await
        .unwrap();

    // Profile is deleted
    assert_err!(Profile::get_by_id(&db, &profile.id).await);
}

#[driver_test(id(ID))]
pub async fn unset_has_one_with_required_pair_in_non_pk_query_update(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        email: String,

        #[has_one]
        profile: toasty::HasOne<Option<Profile>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let db = test.setup_db(models!(User, Profile)).await;

    let user = User::create()
        .email("foo@example.com")
        .profile(Profile::create())
        .exec(&db)
        .await
        .unwrap();
    let profile = user.profile().get(&db).await.unwrap().unwrap();
    assert_eq!(profile.user_id, user.id);

    User::filter_by_email(&user.email)
        .update()
        .profile(None)
        .exec(&db)
        .await
        .unwrap();

    // Profile is deleted
    assert_err!(Profile::get_by_id(&db, &profile.id).await);
}

#[driver_test(id(ID))]
pub async fn associate_has_one_by_val_on_insert(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_one]
        profile: toasty::HasOne<Profile>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        bio: String,
    }

    let db = test.setup_db(models!(User, Profile)).await;

    // Create a profile
    let profile = Profile::create()
        .bio("hello world")
        .exec(&db)
        .await
        .unwrap();

    // Create a user and associate the profile with it, by value
    let u1 = User::create().profile(&profile).exec(&db).await.unwrap();

    let profile_reloaded = u1.profile().get(&db).await.unwrap();
    assert_eq!(profile.id, profile_reloaded.id);
    assert_eq!(Some(&u1.id), profile_reloaded.user_id.as_ref());
    assert_eq!(profile.bio, profile_reloaded.bio);
}

#[driver_test(id(ID))]
#[ignore]
pub async fn associate_has_one_by_val_on_update_query_with_filter(_test: &mut Test) {
    /*
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

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,

        bio: String,
    }

    let db = test.setup_db(models!(User, Profile)).await;

    let u1 = User::create().name("user 1").exec(&db).await.unwrap();
    let p1 = Profile::create()
        .bio("hello world")
        .exec(&db)
        .await
        .unwrap();

    User::filter_by_id(&u1.id)
        .update()
        .profile(&p1)
        .exec(&db)
        .await
        .unwrap();

    let u1_reloaded = User::get_by_id(&db, &u1.id).await.unwrap();
    let p1_reloaded = u1_reloaded.profile().get(&db).await.unwrap().unwrap();
    assert_eq!(p1.id, p1_reloaded.id);
    assert_eq!(p1.bio, p1_reloaded.bio);
    assert_eq!(p1_reloaded.user_id.as_ref(), Some(&u1.id));

    // Unset
    User::filter_by_id(&u1.id)
        .update()
        .profile(None)
        .exec(&db)
        .await
        .unwrap();

    // Getting this to work will require a big chunk of work in the planner.
    User::filter_by_id(&u1.id)
        .filter(User::FIELDS.name().eq("anon"))
        .update()
        .profile(&p1)
        .exec(&db)
        .await
        .unwrap();
    */
}
