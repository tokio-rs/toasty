use crate::prelude::*;

use toasty_core::{
    driver::Operation,
    stmt::{Expr, ExprSet, InsertTarget, Statement},
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

    let mut db = test.setup_db(models!(User)).await;

    let u = User::create().name("foo").exec(&mut db).await?;
    assert_eq!(u.name, "foo");

    // Verify that the INSERT operation used the correct column name "my_name"
    // and sent the value as a string
    let (op, _resp) = test.log().pop();

    // Get the expected column IDs for the users table
    let user_table_id = table_id(&db, "users");
    let expected_columns = columns(&db, "users", &["id", "my_name"]);

    // Verify the operation uses the correct table and column names, and that
    // the value is transmitted either as a bind parameter (SQL) or inline (DDB).
    //
    // Position: id_u64 uses Expr::Default for auto-increment (no param), so "foo"
    // is at params[0]. id_uuid generates the uuid client-side, so "foo" is at
    // params[1].
    let sql = test.capability().sql;
    let val_pos = if driver_test_cfg!(id_u64) { 0 } else { 1 };
    let val = if sql {
        ArgOr::Arg(val_pos)
    } else {
        ArgOr::Value("foo")
    };
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Insert({
            target: InsertTarget::Table({
                table: == user_table_id,
                columns: == expected_columns,
            }),
            source.body: ExprSet::Values({
                rows: [=~ (Any, val)],
            }),
        }),
    }));
    if sql {
        assert_struct!(op, Operation::QuerySql({
            params[val_pos].value: == "foo",
        }));
    }
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

    let mut db = test.setup_db(models!(User)).await;

    let u = User::create().name("foo").exec(&mut db).await?;
    assert_eq!(u.name, "foo");

    // Verify that the INSERT operation used the correct column name "my_name"
    // and sent the value as a string
    let (op, _resp) = test.log().pop();

    // Get the expected column IDs for the users table
    let user_table_id = table_id(&db, "users");
    let expected_columns = columns(&db, "users", &["id", "my_name"]);

    // Verify the operation uses the correct table and column names, and that the
    // value "foo" is sent as a string bind parameter. This test is SQL-only
    // (requires native_varchar), so the value always becomes an Arg placeholder.
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Insert({
            target: InsertTarget::Table({
                table: == user_table_id,
                columns: == expected_columns,
            }),
            source.body: ExprSet::Values({
                rows: [Expr::Record({ fields: [_, Expr::Arg(_)] })],
            }),
        }),
        params: [.., { value: == "foo" }],
    }));

    // Creating a user with a name larger than 5 characters should fail.
    let res = User::create().name("foo bar").exec(&mut db).await;
    assert!(res.is_err());
    Ok(())
}
