use crate::prelude::*;

use toasty_core::{
    driver::Operation,
    stmt::{ExprSet, InsertTarget, Statement},
};

#[driver_test(id(ID), requires(native_varchar))]
pub async fn specify_constrained_string_field(test: &mut Test) -> Result<()> {
    #[derive(toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[column(type = varchar(5))]
        name: String,
    }

    let db = test.setup_db(models!(User)).await;

    let u = User::create().name("foo").exec(&db).await?;
    assert_eq!(u.name, "foo");

    // Creating a user with a name larger than 5 characters should fail.
    let res = User::create().name("foo bar").exec(&db).await;
    assert!(res.is_err());
    Ok(())
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
        format!("unsupported feature: VARCHAR(1000000000000) exceeds database maximum of {max}")
    );
}

#[driver_test(id(ID), requires(not(native_varchar)))]
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
    assert_eq!(
        err.to_string(),
        "unsupported feature: VARCHAR type is not supported by this database"
    );
}

#[driver_test(id(ID))]
pub async fn specify_uuid_as_text(test: &mut Test) -> Result<()> {
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
        let val_str = val.to_string();
        let created = Foo::create().val(val).exec(&db).await?;

        // Verify that the INSERT operation stored the UUID as a text string
        let (op, _resp) = test.log().pop();

        // Verify the operation uses the correct table and columns,
        // and the UUID value is sent as a string
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Insert(_ {
                target: InsertTarget::Table(_ {
                    table: == table_id(&db, "foos"),
                    columns: == columns(&db, "foos", &["id", "val"]),
                    ..
                }),
                source.body: ExprSet::Values(_ {
                    rows: [=~ (Any, val_str)],
                    ..
                }),
                ..
            }),
            ..
        }));

        let read = Foo::get_by_id(&db, &created.id).await?;
        assert_eq!(read.val, val);

        let (op, _) = test.log().pop();

        if test.capability().sql {
            assert_struct!(op, Operation::QuerySql(_ {
                stmt: Statement::Query(_),
                ..
            }))
        } else {
            assert_struct!(op, Operation::GetByKey(_))
        }
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn specify_uuid_as_bytes(test: &mut Test) -> Result<()> {
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
        let created = Foo::create().val(val).exec(&db).await?;
        let read = Foo::get_by_id(&db, &created.id).await?;
        assert_eq!(read.val, val);
    }
    Ok(())
}
