use crate::prelude::*;

use toasty_core::{
    driver::{Operation, Rows},
    stmt::{Source, Statement, UpdateTarget},
};

#[driver_test(id(ID))]
pub async fn crud_no_fields(t: &mut Test) -> Result<()> {
    const MORE: i32 = 10;

    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,
    }

    let db = t.setup_db(models!(Foo)).await;

    let created = Foo::create().exec(&db).await?;

    // Find Foo
    let read = Foo::filter_by_id(created.id)
        .all(&db)
        .await?
        .collect::<Vec<_>>()
        .await?;

    assert_eq!(1, read.len());
    assert_eq!(created.id, read[0].id);

    // Generate a few instances, IDs should be different

    let mut ids = vec![];

    for _ in 0..MORE {
        let item = Foo::create().exec(&db).await?;
        assert_ne!(item.id, created.id);
        ids.push(item.id);
    }

    assert_unique!(ids);

    for id in &ids {
        let read = Foo::filter_by_id(id)
            .all(&db)
            .await?
            .collect::<Vec<_>>()
            .await?;

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
            let val = Foo::get_by_id(&db, &id).await?;
            val.delete(&db).await?;
        } else {
            // Delete by ID
            Foo::filter_by_id(id).delete(&db).await?;
        }

        // Assert deleted
        assert_err!(Foo::get_by_id(&db, id).await);

        // Assert other foos remain
        for id in &ids {
            let item = Foo::get_by_id(&db, id).await?;
            assert_eq!(*id, item.id);
        }
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn crud_one_string(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        val: String,
    }

    let db = test.setup_db(models!(Foo)).await;

    let foo_table_id = table_id(&db, "foos");
    let is_sql = test.capability().sql;

    let mut created = Foo::create().val("hello world").exec(&db).await?;

    assert_eq!(created.val, "hello world");

    // Find Foo
    let read = Foo::filter_by_id(created.id)
        .all(&db)
        .await?
        .collect::<Vec<_>>()
        .await?;

    assert_eq!(1, read.len());
    assert_eq!(created.id, read[0].id);
    assert_eq!(created.val, "hello world");

    let mut ids = vec![];

    for i in 0..10 {
        let item = Foo::create().val(format!("hello {i}")).exec(&db).await?;

        assert_ne!(item.id, created.id);
        ids.push(item.id);
    }

    assert_unique!(ids);

    for (i, id) in ids.iter().enumerate() {
        let read = Foo::filter_by_id(id)
            .all(&db)
            .await?
            .collect::<Vec<_>>()
            .await?;

        assert_eq!(1, read.len());
        assert_eq!(*id, read[0].id);
        assert_eq!(format!("hello {i}"), read[0].val);
    }

    // Update by val (instance method — generates full-key filter).
    test.log().clear();
    created.update().val("updated!").exec(&db).await?;
    assert_eq!(created.val, "updated!");

    let (op, resp) = test.log().pop();
    // Column index 1 = val (id=0, val=1).
    if is_sql {
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Update(_ {
                target: UpdateTarget::Table(== foo_table_id),
                assignments: #{ 1: _ { expr: == "updated!", .. }},
                ..
            }),
            ret: None,
            ..
        }));
    } else {
        assert_struct!(op, Operation::UpdateByKey(_ {
            table: == foo_table_id,
            keys.len(): 1,
            assignments: #{ 1: _ { expr: == "updated!", .. }},
            filter: None,
            returning: false,
            ..
        }));
    }
    assert_struct!(resp, _ { rows: Rows::Count(1), .. });
    assert!(test.log().is_empty());

    test.log().clear();
    let reload = Foo::get_by_id(&db, &created.id).await?;
    assert_eq!(reload.val, created.val);

    // Update by ID
    Foo::filter_by_id(created.id)
        .update()
        .val("updated again!")
        .exec(&db)
        .await?;
    let reload = Foo::get_by_id(&db, &created.id).await?;
    assert_eq!(reload.val, "updated again!");

    // Delete the record (instance method — generates full-key filter).
    test.log().clear();
    reload.delete(&db).await?;

    let (op, resp) = test.log().pop();
    if is_sql {
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Delete(_ {
                from: Source::Table(_ {
                    tables: [== foo_table_id, ..],
                    ..
                }),
                ..
            }),
            ..
        }));
    } else {
        assert_struct!(op, Operation::DeleteByKey(_ {
            table: == foo_table_id,
            keys.len(): 1,
            filter: None,
            ..
        }));
    }
    assert_struct!(resp, _ { rows: Rows::Count(1), .. });
    assert!(test.log().is_empty());

    // It is gone
    assert_err!(Foo::get_by_id(&db, &created.id).await);

    // Delete by ID
    Foo::filter_by_id(ids[0]).delete(&db).await?;

    // It is gone
    assert_err!(Foo::get_by_id(&db, &ids[0]).await);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn required_field_create_without_setting(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[allow(dead_code)]
        name: String,
    }

    let db = test.setup_db(models!(User)).await;

    // Try creating a user without setting the name field results in an error
    assert_err!(User::create().exec(&db).await);
}

#[driver_test(id(ID))]
pub async fn unique_index_required_field_update(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        email: String,
    }

    let db = test.setup_db(models!(User)).await;

    let email = "user1@example.com";

    let user = User::create().email(email).exec(&db).await?;

    assert_eq!("user1@example.com", user.email);

    // Trying to create a user with the same email address fails
    assert_err!(User::create().email(email).exec(&db).await);

    // Loading the user by email
    let user_reloaded = User::get_by_email(&db, email).await?;
    assert_eq!(user.id, user_reloaded.id);
    assert_eq!(user_reloaded.email, email);

    // Creating a user with a different email works
    let user_alt_email = User::create()
        .email("alt-email@example.com")
        .exec(&db)
        .await?;

    assert_ne!(user.id, user_alt_email.id);

    // Deleting the user then reuse the email address
    user.delete(&db).await?;

    // Finding by the email returns None
    assert_none!(User::filter_by_email(email).first(&db).await?);

    let mut user2 = User::create().email(email).exec(&db).await?;
    assert_ne!(user2.id, user_reloaded.id);

    // Trying to create a third user with that email address fails.
    assert_err!(User::create().email(email).exec(&db).await);

    // Updating the email address by object
    user2.update().email("user2@example.com").exec(&db).await?;

    // Reload the user by ID
    let user_reloaded = User::filter_by_id(user2.id).get(&db).await?;
    assert_eq!(user2.id, user_reloaded.id);
    assert_eq!(user_reloaded.email, "user2@example.com");

    // Finding by the email returns None
    assert_none!(User::filter_by_email(email).first(&db).await?);

    // Trying to create a user with the updated email address fails
    assert_err!(User::create().email("user2@example.com").exec(&db).await);

    // Creating a user with the **old** email address succeeds
    let user3 = User::create().email(email).exec(&db).await?;
    assert_eq!(user3.email, email);
    assert_ne!(user3.id, user2.id);

    // Updating the email address by ID
    User::filter_by_id(user2.id)
        .update()
        .email("user3@example.com")
        .exec(&db)
        .await?;

    // Finding by the email returns None
    assert_none!(User::filter_by_email(&user2.email).first(&db).await?);

    // Find the user by the new address.
    let u = User::filter_by_email("user3@example.com").get(&db).await?;

    assert_eq!(u.id, user2.id);

    assert_err!(User::create().email("user3@example.com").exec(&db).await);

    // But we *can* create a user w/ the old email
    assert_ok!(User::create().email("user2@example.com").exec(&db).await);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn unique_index_nullable_field_update(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        email: Option<String>,
    }

    let db = test.setup_db(models!(User)).await;

    // Create a user without an email address
    let mut u1 = User::create().exec(&db).await?;
    assert!(u1.email.is_none());

    // Create a second user without an email address
    let mut u2 = User::create().exec(&db).await?;
    assert!(u2.email.is_none());

    // Reload u1 and make sure email is still set.
    let u1_reload = User::get_by_id(&db, &u1.id).await?;
    assert!(u1_reload.email.is_none());

    // Finding by a bogus email finds nothing
    assert_none!(User::filter_by_email("foo@example.com").first(&db).await?);

    // Create a user **with** an email
    let mut u3 = User::create().email("three@example.com").exec(&db).await?;
    assert_eq!(u3.email, Some("three@example.com".to_string()));

    let u3_reload = User::get_by_email(&db, "three@example.com").await?;
    assert_eq!(u3_reload.id, u3.id);

    // Now, set u1's email to something
    u1.update().email("one@example.com").exec(&db).await?;
    assert_eq!(u1.email, Some("one@example.com".to_string()));

    // Find it
    let u1_reload = User::get_by_email(&db, "one@example.com").await?;
    assert_eq!(u1.id, u1_reload.id);

    // Try updating user 2 to an already taken email address
    assert_err!(u2.update().email("three@example.com").exec(&db).await);

    // Can still fetch user 3
    let u3_reload = User::get_by_email(&db, "three@example.com").await?;
    assert_eq!(u3_reload.id, u3.id);

    // Update user 2 to set an actual email now.
    u2.update().email("two@example.com").exec(&db).await?;
    let u2_reload = User::get_by_email(&db, "two@example.com").await?;
    assert_eq!(u2_reload.id, u2.id);

    // Update a user to **remove** the email attribute
    let mut update = u3.update();
    update.set_email(None);
    update.exec(&db).await?;
    assert!(u3.email.is_none());

    // We can create a new user using the freed email
    let u4 = User::create().email("three@example.com").exec(&db).await?;
    let u4_reload = User::filter_by_email("three@example.com").get(&db).await?;
    assert_eq!(u4_reload.id, u4.id);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn unique_index_no_update(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        email: String,

        name: String,
    }

    let db = test.setup_db(models!(User)).await;

    let mut user = User::create()
        .email("user@example.com")
        .name("John Doe")
        .exec(&db)
        .await?;

    let u = User::filter_by_id(user.id).get(&db).await?;
    assert_eq!(user.name, u.name);

    // Update the name by value
    user.update().name("Jane Doe").exec(&db).await?;

    assert_eq!("Jane Doe", user.name);

    let u = User::get_by_id(&db, &user.id).await?;
    assert_eq!(user.name, u.name);

    // Find by email still works
    let u = User::get_by_email(&db, &user.email).await?;
    assert_eq!(user.name, u.name);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn batch_get_by_id(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,
    }

    let db = test.setup_db(models!(Foo)).await;
    let mut keys = vec![];

    for _ in 0..5 {
        let item = Foo::create().exec(&db).await?;
        keys.push(item.id);
    }

    let items: Vec<_> = Foo::filter_by_id_batch([&keys[0], &keys[1], &keys[2]])
        .collect(&db)
        .await?;

    assert_eq!(3, items.len());

    for item in items {
        assert!(keys.contains(&item.id));
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn empty_batch_get_by_id(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,
    }

    let db = test.setup_db(models!(Foo)).await;
    let mut ids = vec![];

    for _ in 0..5 {
        let item = Foo::create().exec(&db).await?;
        ids.push(item.id);
    }

    let items: Vec<_> = Foo::filter_by_id_batch(&[] as &[ID]).collect(&db).await?;

    assert_eq!(0, items.len());
    Ok(())
}

#[driver_test(id(ID))]
pub async fn update_multiple_fields(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,
        email: String,
    }

    let db = test.setup_db(models!(User)).await;

    let mut user = User::create()
        .name("John Doe")
        .email("john@example.com")
        .exec(&db)
        .await?;

    // Update by object
    user.update()
        .name("Jane Doe")
        .email("jane@example.com")
        .exec(&db)
        .await?;

    assert_eq!("Jane Doe", user.name);
    assert_eq!("jane@example.com", user.email);

    let user = User::get_by_id(&db, &user.id).await?;
    assert_eq!("Jane Doe", user.name);
    assert_eq!("jane@example.com", user.email);

    // Update by query
    User::filter_by_id(user.id)
        .update()
        .name("John2 Doe")
        .email("john2@example.com")
        .exec(&db)
        .await?;

    let user = User::get_by_id(&db, &user.id).await?;
    assert_eq!("John2 Doe", user.name);
    assert_eq!("john2@example.com", user.email);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn update_and_delete_snippets(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[allow(dead_code)]
        name: String,
    }

    let db = test.setup_db(models!(User)).await;

    let user = User::create().name("John Doe").exec(&db).await?;

    User::update_by_id(user.id)
        .name("John Doe2")
        .exec(&db)
        .await?;

    let new_user = User::get_by_id(&db, user.id).await?;
    assert!(new_user.name == "John Doe2");

    User::delete_by_id(&db, user.id).await?;

    assert_err!(User::get_by_id(&db, user.id).await);
    Ok(())
}
