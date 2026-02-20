use crate::prelude::*;

use toasty_core::{
    driver::Operation,
    stmt::{ExprSet, InsertTarget, Statement},
};

#[driver_test(id(ID))]
pub async fn specify_custom_column_name(test: &mut Test) -> Result<()> {
    #[derive(toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[column("my_name")]
        name: String,
    }

    let db = test.setup_db(models!(User)).await;

    let u = User::create().name("foo").exec(&db).await?;
    assert_eq!(u.name, "foo");

    // Verify that the INSERT operation used the correct column name "my_name"
    // and sent the value as a string
    let (op, _resp) = test.log().pop();

    // Get the expected column IDs for the users table
    let user_table_id = table_id(&db, "users");
    let expected_columns = columns(&db, "users", &["id", "my_name"]);

    // Verify the operation uses the correct table and column names
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Insert(_ {
            target: InsertTarget::Table(_ {
                table: == user_table_id,
                columns: == expected_columns,
                ..
            }),
            source.body: ExprSet::Values(_ {
                rows: [=~ (Any, "foo")],
                ..
            }),
            ..
        }),
        ..
    }));
    Ok(())
}

#[driver_test(id(ID), requires(native_varchar))]
pub async fn specify_custom_column_name_with_type(test: &mut Test) -> Result<()> {
    #[derive(toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[column("my_name", type = varchar(5))]
        name: String,
    }

    let db = test.setup_db(models!(User)).await;

    let u = User::create().name("foo").exec(&db).await?;
    assert_eq!(u.name, "foo");

    // Verify that the INSERT operation used the correct column name "my_name"
    // and sent the value as a string
    let (op, _resp) = test.log().pop();

    // Get the expected column IDs for the users table
    let user_table_id = table_id(&db, "users");
    let expected_columns = columns(&db, "users", &["id", "my_name"]);

    // Verify the operation uses the correct table and column names
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Insert(_ {
            target: InsertTarget::Table(_ {
                table: == user_table_id,
                columns: == expected_columns,
                ..
            }),
            ..
        }),
        ..
    }));

    // Verify the value "foo" is sent as a string
    if let Operation::QuerySql(query) = op {
        if let Statement::Insert(insert) = query.stmt {
            if let ExprSet::Values(values) = insert.source.body {
                assert_struct!(values.rows, [=~ (Any, "foo")]);
            } else {
                panic!("Expected Values in INSERT source");
            }
        }
    }

    // Creating a user with a name larger than 5 characters should fail.
    let res = User::create().name("foo bar").exec(&db).await;
    assert!(res.is_err());
    Ok(())
}
