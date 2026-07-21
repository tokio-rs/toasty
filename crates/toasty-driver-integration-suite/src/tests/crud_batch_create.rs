//! Test batch creation of models

use crate::prelude::*;

use toasty_core::{
    driver::{Operation, operation::Transaction},
    schema::db,
    stmt::{Expr, ExprFunc, ExprSet, Statement, TableRef, Value},
};

#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
pub async fn batch_create_empty(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let res = Post::create_many().exec(&mut db).await?;
    assert!(res.is_empty());
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
pub async fn batch_create_one(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    test.log().clear();
    let res = Post::create_many()
        .item(Post::create().title("hello"))
        .exec(&mut db)
        .await?;

    assert_eq!(1, res.len());
    assert_eq!(res[0].title, "hello");

    // Single-row batch: no transaction wrapping needed
    if test.capability().sql {
        assert_struct!(test.log().pop_op(), Operation::QuerySql(_));
        assert!(test.log().is_empty());
    }

    let reloaded: Vec<_> = Post::filter_by_id(res[0].id).exec(&mut db).await?;
    assert_eq!(1, reloaded.len());
    assert_eq!(reloaded[0].id, res[0].id);
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
pub async fn batch_create_many(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    test.log().clear();
    let res = Post::create_many()
        .item(Post::create().title("todo 1"))
        .item(Post::create().title("todo 2"))
        .exec(&mut db)
        .await?;

    assert_eq!(2, res.len());
    assert_eq!(res[0].title, "todo 1");
    assert_eq!(res[1].title, "todo 2");

    // Multi-row batch in a single INSERT statement: no transaction wrapping
    // needed because single SQL statements are inherently atomic.
    if test.capability().sql {
        assert_struct!(test.log().pop_op(), Operation::QuerySql(_));
        assert!(test.log().is_empty());
    }

    for post in &res {
        let reloaded: Vec<_> = Post::filter_by_id(post.id).exec(&mut db).await?;
        assert_eq!(1, reloaded.len());
        assert_eq!(reloaded[0].id, post.id);
    }
    Ok(())
}

/// On PostgreSQL this exercises the INSERT → `unnest` transpose with a NULL
/// cell inside a column array bind.
#[driver_test(id(ID))]
pub async fn batch_create_with_null_field(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        name: Option<String>,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let res = Item::create_many()
        .item(Item::create().name("n1"))
        .item(Item::create())
        .item(Item::create().name("n3"))
        .exec(&mut db)
        .await?;

    assert_eq!(3, res.len());
    assert_eq!(res[0].name.as_deref(), Some("n1"));
    assert_eq!(res[1].name, None);
    assert_eq!(res[2].name.as_deref(), Some("n3"));

    let reloaded = Item::get_by_id(&mut db, &res[1].id).await?;
    assert_eq!(reloaded.name, None);
    Ok(())
}

#[driver_test(requires(insert_values_unnest))]
pub async fn batch_create_uses_unnest_array_params(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: String,
        name: Option<String>,
    }

    let mut db = test.setup_db(models!(Item)).await;

    test.log().clear();
    Item::create_many()
        .item(Item::create().id("item-1").name("n1"))
        .item(Item::create().id("item-2"))
        .exec(&mut db)
        .await?;

    let Operation::QuerySql(query) = test.log().pop_op() else {
        panic!("expected QuerySql operation");
    };
    let Statement::Insert(insert) = query.stmt else {
        panic!("expected Insert statement");
    };
    let ExprSet::Select(select) = insert.source.body else {
        panic!("expected Select insert source");
    };
    let [TableRef::RowsFrom(funcs)] = select.source.as_table_unwrap().tables.as_slice() else {
        panic!("expected ROWS FROM table source");
    };
    let [ExprFunc::Unnest(ids), ExprFunc::Unnest(names)] = funcs.as_slice() else {
        panic!("expected one unnest function per inserted column");
    };

    assert_eq!(ids.arg.as_ref(), &Expr::arg(0));
    assert_eq!(names.arg.as_ref(), &Expr::arg(1));
    assert_eq!(query.params.len(), 2);
    assert_eq!(
        query.params[0].value,
        Value::List(vec![Value::from("item-1"), Value::from("item-2")])
    );
    assert_eq!(query.params[0].ty, db::Type::list(db::Type::Text));
    assert_eq!(
        query.params[1].value,
        Value::List(vec![Value::from("n1"), Value::Null])
    );
    assert_eq!(query.params[1].ty, db::Type::list(db::Type::Text));
    assert!(test.log().is_empty());

    Ok(())
}

// TODO: is a batch supposed to be atomic? Probably not.
#[driver_test(id(ID))]
#[should_panic]
pub async fn batch_create_fails_if_any_record_missing_fields(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        email: String,

        #[allow(dead_code)]
        name: String,
    }

    let mut db = test.setup_db(models!(User)).await;

    let res = User::create_many()
        .item(User::create().email("user1@example.com").name("User 1"))
        .item(User::create().email("user2@example.com"))
        .exec(&mut db)
        .await?;

    assert!(res.is_empty());

    let users: Vec<_> = User::filter_by_email("me@carllerche.com")
        .exec(&mut db)
        .await?;

    assert!(users.is_empty());
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::user_unique_email))]
pub async fn batch_create_model_with_unique_field_index_all_unique(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut res = User::create_many()
        .item(User::create().email("user1@example.com"))
        .item(User::create().email("user2@example.com"))
        .exec(&mut db)
        .await?;

    assert_eq!(2, res.len());

    res.sort_by_key(|user| user.email.clone());

    assert_eq!(res[0].email, "user1@example.com");
    assert_eq!(res[1].email, "user2@example.com");

    // We can fetch the user by ID and email
    for user in &res {
        let found = User::get_by_id(&mut db, user.id).await?;
        assert_eq!(found.id, user.id);
        assert_eq!(found.email, user.email);

        let found = User::get_by_email(&mut db, &user.email).await?;
        assert_eq!(found.id, user.id);
        assert_eq!(found.email, user.email);
    }
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::user_unique_email))]
#[should_panic]
pub async fn batch_create_model_with_unique_field_index_all_dups(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let _res = User::create_many()
        .item(User::create().email("user@example.com"))
        .item(User::create().email("user@example.com"))
        .exec(&mut db)
        .await?;
    Ok(())
}

/// Unique constraint violation on a multi-row batch is atomic because a single
/// INSERT statement is inherently atomic in SQL databases.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::user_unique_email))]
pub async fn batch_create_unique_violation_rolls_back(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    // Seed the duplicate
    User::create()
        .email("taken@example.com")
        .exec(&mut db)
        .await?;

    t.log().clear();
    assert_err!(
        User::create_many()
            .item(User::create().email("new@example.com"))
            .item(User::create().email("taken@example.com"))
            .exec(&mut db)
            .await
    );

    // No transaction wrapper — the single INSERT fails atomically
    assert!(t.log().is_empty());

    // Only the seeded user remains
    let users = User::all().exec(&mut db).await?;
    assert_eq!(1, users.len());

    Ok(())
}

/// Multi-row batch inside an explicit transaction executes as a single INSERT
/// without extra savepoint wrapping (the statement is inherently atomic).
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn batch_create_inside_transaction_uses_savepoints(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    t.log().clear();
    let mut tx = db.transaction().await?;

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            isolation: None,
            read_only: false,
            ..
        })
    );

    Post::create_many()
        .item(Post::create().title("a"))
        .item(Post::create().title("b"))
        .exec(&mut tx)
        .await?;

    // Single INSERT statement — no savepoint needed
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_));

    tx.commit().await?;

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Commit)
    );
    assert!(t.log().is_empty());

    Ok(())
}
