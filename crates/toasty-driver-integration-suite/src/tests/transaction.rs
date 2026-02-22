use crate::prelude::*;
use std::time::Duration;

#[driver_test(id(ID))]
pub async fn basic_commit(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        val: String,
    }

    let db = t.setup_db(models!(Foo)).await;

    db.transaction(async |tx| {
        Foo::create().val("hello").exec(tx).await?;
        Ok::<(), toasty::Error>(())
    })
    .await?;

    let foos: Vec<Foo> = Foo::all().collect(&db).await?;
    assert_eq!(foos.len(), 1);
    assert_eq!(foos[0].val, "hello");
    Ok(())
}

#[driver_test(id(ID))]
pub async fn rollback_on_error(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        val: String,
    }

    let db = t.setup_db(models!(Foo)).await;

    let result: Result<()> = db
        .transaction(async |tx| {
            Foo::create().val("hello").exec(tx).await?;
            Err(toasty::Error::invalid_result("deliberate rollback"))
        })
        .await;

    assert!(result.is_err());

    let foos: Vec<Foo> = Foo::all().collect(&db).await?;
    assert_eq!(foos.len(), 0);
    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn timeout_rollback(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        val: String,
    }

    let db = t.setup_db(models!(Foo)).await;

    let result: Result<()> = db
        .transaction_builder()
        .timeout(Duration::from_millis(10))
        .exec(async |tx| {
            Foo::create().val("hello").exec(tx).await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(())
        })
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("timed out"));

    let foos: Vec<Foo> = Foo::all().collect(&db).await?;
    assert_eq!(foos.len(), 0);
    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn nested_commit(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        val: String,
    }

    let db = t.setup_db(models!(Foo)).await;

    db.transaction(async |tx| {
        Foo::create().val("outer").exec(tx).await?;

        tx.transaction(async |inner| {
            Foo::create().val("inner").exec(inner).await?;
            Ok::<(), toasty::Error>(())
        })
        .await?;

        Ok::<(), toasty::Error>(())
    })
    .await?;

    let mut foos: Vec<Foo> = Foo::all().collect(&db).await?;
    foos.sort_by(|a, b| a.val.cmp(&b.val));
    assert_eq!(foos.len(), 2);
    assert_eq!(foos[0].val, "inner");
    assert_eq!(foos[1].val, "outer");
    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn nested_inner_commits_outer_fails(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        val: String,
    }

    let db = t.setup_db(models!(Foo)).await;

    // The inner transaction commits (releases its savepoint), but the outer
    // transaction then fails. The outer ROLLBACK undoes everything, including
    // the inner work, because savepoint release does not protect against an
    // enclosing rollback.
    let result: Result<()> = db
        .transaction(async |tx| {
            Foo::create().val("outer").exec(tx).await?;

            tx.transaction(async |inner| {
                Foo::create().val("inner").exec(inner).await?;
                Ok::<(), toasty::Error>(())
            })
            .await?;

            Err(toasty::Error::invalid_result("outer rollback"))
        })
        .await;

    assert!(result.is_err());

    let foos: Vec<Foo> = Foo::all().collect(&db).await?;
    assert_eq!(foos.len(), 0);
    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn nested_rollback(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        val: String,
    }

    let db = t.setup_db(models!(Foo)).await;

    db.transaction(async |tx| {
        Foo::create().val("outer").exec(tx).await?;

        let inner_result: Result<()> = tx
            .transaction(async |inner| {
                Foo::create().val("inner").exec(inner).await?;
                Err(toasty::Error::invalid_result("inner rollback"))
            })
            .await;

        assert!(inner_result.is_err());

        Ok::<(), toasty::Error>(())
    })
    .await?;

    let foos: Vec<Foo> = Foo::all().collect(&db).await?;
    assert_eq!(foos.len(), 1);
    assert_eq!(foos[0].val, "outer");
    Ok(())
}
