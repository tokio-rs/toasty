use tests_client::*;

use std_util::prelude::*;
use toasty::stmt::Id;

const MORE: i32 = 10;

async fn crud_no_fields(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
    }

    let db = s.setup(models!(Foo)).await;

    let created = Foo::create().exec(&db).await.unwrap();

    // Find Foo
    let read = Foo::filter_by_id(&created.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, read.len());
    assert_eq!(created.id, read[0].id);

    // Generate a few instances, IDs should be different

    let mut ids = vec![];

    for _ in 0..MORE {
        let foo = Foo::create().exec(&db).await.unwrap();
        assert_ne!(foo.id, created.id);
        ids.push(foo.id);
    }

    std_util::assert_unique!(ids);

    for id in &ids {
        let read = Foo::filter_by_id(id)
            .all(&db)
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await
            .unwrap();

        assert_eq!(1, read.len());
        assert_eq!(*id, read[0].id);
    }

    // Randomize the IDs
    ids.shuffle();

    // Delete the IDs
    for i in 0..MORE {
        let id = ids.pop().unwrap();

        if i.is_even() {
            // Delete by object
            let val = Foo::get_by_id(&db, &id).await.unwrap();
            val.delete(&db).await.unwrap();
        } else {
            // Delete by ID
            Foo::filter_by_id(&id).delete(&db).await.unwrap();
        }

        // Assert deleted
        assert_err!(Foo::get_by_id(&db, id).await);

        // Assert other foos remain
        for id in &ids {
            let foo = Foo::get_by_id(&db, id).await.unwrap();
            assert_eq!(*id, foo.id);
        }
    }
}

async fn crud_one_string(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,

        val: String,
    }

    let db = s.setup(models!(Foo)).await;

    let mut created = Foo::create().val("hello world").exec(&db).await.unwrap();

    assert_eq!(created.val, "hello world");

    // Find Foo
    let read = Foo::filter_by_id(&created.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, read.len());
    assert_eq!(created.id, read[0].id);
    assert_eq!(created.val, "hello world");

    let mut ids = vec![];

    for i in 0..10 {
        let foo = Foo::create()
            .val(format!("hello {i}"))
            .exec(&db)
            .await
            .unwrap();

        assert_ne!(foo.id, created.id);
        ids.push(foo.id);
    }

    std_util::assert_unique!(ids);

    for (i, id) in ids.iter().enumerate() {
        let read = Foo::filter_by_id(id)
            .all(&db)
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await
            .unwrap();

        assert_eq!(1, read.len());
        assert_eq!(*id, read[0].id);
        assert_eq!(format!("hello {i}"), read[0].val);
    }

    // Update by val
    created.update().val("updated!").exec(&db).await.unwrap();
    assert_eq!(created.val, "updated!");

    let reload = Foo::get_by_id(&db, &created.id).await.unwrap();
    assert_eq!(reload.val, created.val);

    // Update by ID
    Foo::filter_by_id(&created.id)
        .update()
        .val("updated again!")
        .exec(&db)
        .await
        .unwrap();
    let reload = Foo::get_by_id(&db, &created.id).await.unwrap();
    assert_eq!(reload.val, "updated again!");

    // Delete the record
    reload.delete(&db).await.unwrap();

    // It is gone
    assert_err!(Foo::get_by_id(&db, &created.id).await);

    // Delete by ID
    Foo::filter_by_id(&ids[0]).delete(&db).await.unwrap();

    // It is gone
    assert_err!(Foo::get_by_id(&db, &ids[0]).await);
}

async fn required_field_create_without_setting(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,
    }

    let db = s.setup(models!(User)).await;

    // Try creating a user without setting the name field results in an error
    assert_err!(User::create().exec(&db).await);
}

async fn unique_index_required_field_update(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[unique]
        email: String,
    }

    let db = s.setup(models!(User)).await;

    let email = "user1@example.com";

    let user = User::create().email(email).exec(&db).await.unwrap();

    assert_eq!("user1@example.com", user.email);

    // Trying to create a user with the same email address fails
    assert_err!(User::create().email(email).exec(&db).await);

    // Loading the user by email
    let user_reloaded = User::get_by_email(&db, email).await.unwrap();
    assert_eq!(user.id, user_reloaded.id);
    assert_eq!(user_reloaded.email, email);

    // Creating a user with a different email works
    let user_alt_email = User::create()
        .email("alt-email@example.com")
        .exec(&db)
        .await
        .unwrap();

    assert_ne!(user.id, user_alt_email.id);

    // Deleting the user then reuse the email address
    user.delete(&db).await.unwrap();

    // Finding by the email returns None
    assert_none!(User::filter_by_email(email).first(&db).await.unwrap());

    let mut user2 = User::create().email(email).exec(&db).await.unwrap();
    assert_ne!(user2.id, user_reloaded.id);

    // Trying to create a third user with that email address fails.
    assert_err!(User::create().email(email).exec(&db).await);

    // Updating the email address by object
    user2
        .update()
        .email("user2@example.com")
        .exec(&db)
        .await
        .unwrap();

    // Reload the user by ID
    let user_reloaded = User::filter_by_id(&user2.id).get(&db).await.unwrap();
    assert_eq!(user2.id, user_reloaded.id);
    assert_eq!(user_reloaded.email, "user2@example.com");

    // Finding by the email returns None
    assert_none!(User::filter_by_email(email).first(&db).await.unwrap());

    // Trying to create a user with the updated email address fails
    assert_err!(User::create().email("user2@example.com").exec(&db).await);

    // Creating a user with the **old** email address succeeds
    let user3 = User::create().email(email).exec(&db).await.unwrap();
    assert_eq!(user3.email, email);
    assert_ne!(user3.id, user2.id);

    // Updating the email address by ID
    User::filter_by_id(&user2.id)
        .update()
        .email("user3@example.com")
        .exec(&db)
        .await
        .unwrap();

    // Finding by the email returns None
    assert_none!(User::filter_by_email(&user2.email)
        .first(&db)
        .await
        .unwrap());

    // Find the user by the new address.
    let u = User::filter_by_email("user3@example.com")
        .get(&db)
        .await
        .unwrap();

    assert_eq!(u.id, user2.id);

    assert_err!(User::create().email("user3@example.com").exec(&db).await);

    // But we *can* create a user w/ the old email
    assert_ok!(User::create().email("user2@example.com").exec(&db).await);
}

async fn unique_index_nullable_field_update(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[unique]
        email: Option<String>,
    }

    let db = s.setup(models!(User)).await;

    // Create a user without an email address
    let mut u1 = User::create().exec(&db).await.unwrap();
    assert!(u1.email.is_none());

    // Create a second user without an email address
    let mut u2 = User::create().exec(&db).await.unwrap();
    assert!(u2.email.is_none());

    // Reload u1 and make sure email is still set.
    let u1_reload = User::get_by_id(&db, &u1.id).await.unwrap();
    assert!(u1_reload.email.is_none());

    // Finding by a bogus email finds nothing
    assert_none!(User::filter_by_email("foo@example.com")
        .first(&db)
        .await
        .unwrap());

    // Create a user **with** an email
    let mut u3 = User::create()
        .email("three@example.com")
        .exec(&db)
        .await
        .unwrap();
    assert_eq!(u3.email, Some("three@example.com".to_string()));

    let u3_reload = User::get_by_email(&db, "three@example.com").await.unwrap();
    assert_eq!(u3_reload.id, u3.id);

    // Now, set u1's email to something
    u1.update()
        .email("one@example.com")
        .exec(&db)
        .await
        .unwrap();
    assert_eq!(u1.email, Some("one@example.com".to_string()));

    // Find it
    let u1_reload = User::get_by_email(&db, "one@example.com").await.unwrap();
    assert_eq!(u1.id, u1_reload.id);

    // Try updating user 2 to an already taken email address
    assert_err!(u2.update().email("three@example.com").exec(&db).await);

    // Can still fetch user 3
    let u3_reload = User::get_by_email(&db, "three@example.com").await.unwrap();
    assert_eq!(u3_reload.id, u3.id);

    // Update user 2 to set an actual email now.
    u2.update()
        .email("two@example.com")
        .exec(&db)
        .await
        .unwrap();
    let u2_reload = User::get_by_email(&db, "two@example.com").await.unwrap();
    assert_eq!(u2_reload.id, u2.id);

    // Update a user to **remove** the email attribute
    let mut update = u3.update();
    update.set_email(None);
    update.exec(&db).await.unwrap();
    assert!(u3.email.is_none());

    // We can create a new user using the freed email
    let u4 = User::create()
        .email("three@example.com")
        .exec(&db)
        .await
        .unwrap();
    let u4_reload = User::filter_by_email("three@example.com")
        .get(&db)
        .await
        .unwrap();
    assert_eq!(u4_reload.id, u4.id);
}

async fn unique_index_no_update(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[unique]
        email: String,

        name: String,
    }

    let db = s.setup(models!(User)).await;

    let mut user = User::create()
        .email("user@example.com")
        .name("John Doe")
        .exec(&db)
        .await
        .unwrap();

    let u = User::filter_by_id(&user.id).get(&db).await.unwrap();
    assert_eq!(user.name, u.name);

    // Update the name by value
    user.update().name("Jane Doe").exec(&db).await.unwrap();

    assert_eq!("Jane Doe", user.name);

    let u = User::get_by_id(&db, &user.id).await.unwrap();
    assert_eq!(user.name, u.name);

    // Find by email still works
    let u = User::get_by_email(&db, &user.email).await.unwrap();
    assert_eq!(user.name, u.name);
}

async fn batch_get_by_id(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
    }

    let db = s.setup(models!(Foo)).await;
    let mut keys = vec![];

    for _ in 0..5 {
        let foo = Foo::create().exec(&db).await.unwrap();
        keys.push(foo.id);
    }

    let foos: Vec<_> = Foo::filter_by_id_batch(&[&keys[0], &keys[1], &keys[2]])
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(3, foos.len());

    for foo in foos {
        assert!(keys.iter().any(|key| foo.id == *key));
    }
}

async fn empty_batch_get_by_id(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
    }

    let db = s.setup(models!(Foo)).await;
    let mut ids = vec![];

    for _ in 0..5 {
        let foo = Foo::create().exec(&db).await.unwrap();
        ids.push(foo.id);
    }

    let foos: Vec<_> = Foo::filter_by_id_batch(&[] as &[toasty::stmt::Id<Foo>])
        .collect(&db)
        .await
        .unwrap();

    assert_eq!(0, foos.len());
}

tests!(
    crud_no_fields,
    crud_one_string,
    unique_index_required_field_update,
    unique_index_nullable_field_update,
    batch_get_by_id,
    empty_batch_get_by_id,
    unique_index_no_update,
    // TODO: this should be an error, but right now it panics
    #[should_panic]
    required_field_create_without_setting,
);
