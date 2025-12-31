use crate::prelude::*;

#[driver_test(id(ID), requires(native_varchar))]
pub async fn specify_constrained_string_field(test: &mut Test) {
    #[derive(toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[column(type = varchar(5))]
        name: String,
    }

    let db = test.setup_db(models!(User)).await;

    let u = User::create().name("foo").exec(&db).await.unwrap();
    assert_eq!(u.name, "foo");

    // Creating a user with a name larger than 5 characters should fail.
    let res = User::create().name("foo bar").exec(&db).await;
    assert!(res.is_err());
}

#[driver_test(id(ID), requires(native_varchar))]
pub async fn specify_invalid_varchar_size(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[column(type = varchar(1_000_000_000_000))]
        name: String,
    }

    let Some(max) = test.capability().storage_types.varchar else {
        return;
    };

    if max >= 1_000_000_000_000 {
        return;
    }

    // Try to setup a database with an invalid varchar size
    let err = assert_err!(test.try_setup_db(models!(User)).await);
    assert_eq!(
        err.to_string(),
        format!("max varchar capacity exceeded: 1000000000000 > {max}")
    );
}

#[driver_test(id(ID), requires(native_varchar))]
pub async fn specify_varchar_ty_when_not_supported(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[column(type = varchar(5))]
        name: String,
    }

    // Try to setup a database with varchar when not supported
    let err = assert_err!(test.try_setup_db(models!(User)).await);
    assert_eq!(err.to_string(), "varchar storage type not supported");
}

#[driver_test(id(ID))]
pub async fn specify_uuid_as_text(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        #[column(type = text)]
        val: uuid::Uuid,
    }

    let db = test.setup_db(models!(Foo)).await;

    for _ in 0..16 {
        let val = uuid::Uuid::new_v4();
        let created = Foo::create().val(val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, val);

        let mut filter = std::collections::HashMap::new();
        filter.insert("id".to_string(), toasty_core::stmt::Value::from(created.id));
        let raw_value = test
            .get_raw_column_value::<String>("foos", "val", filter)
            .await
            .unwrap();
        assert_eq!(raw_value, val.to_string());
    }
}

#[driver_test(id(ID))]
pub async fn specify_uuid_as_bytes(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        #[column(type = blob)]
        val: uuid::Uuid,
    }

    let db = test.setup_db(models!(Foo)).await;

    for _ in 0..16 {
        let val = uuid::Uuid::new_v4();
        let created = Foo::create().val(val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, val);
    }
}
