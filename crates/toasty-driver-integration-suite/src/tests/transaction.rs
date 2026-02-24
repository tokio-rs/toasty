use crate::prelude::*;
use std::time::Duration;

#[driver_test(id(ID))]
pub async fn isolation_level_serializable(t: &mut Test) -> Result<()> {
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
        .serializable()
        .exec(async |tx| {
            Foo::create().val("hello").exec(tx).await?;
            Ok::<(), toasty::Error>(())
        })
        .await;

    if !t.capability().sql {
        assert!(result.unwrap_err().is_unsupported_feature());
        return Ok(());
    }

    result?;
    let foos: Vec<Foo> = Foo::all().collect(&db).await?;
    assert_eq!(foos.len(), 1);
    assert_eq!(foos[0].val, "hello");
    Ok(())
}

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

    let result: Result<()> = db
        .transaction(async |tx| {
            Foo::create().val("hello").exec(tx).await?;
            Ok::<(), toasty::Error>(())
        })
        .await;

    if !t.capability().sql {
        assert!(result.is_err());
        return Ok(());
    }

    result?;
    let foos: Vec<Foo> = Foo::all().collect(&db).await?;
    assert_eq!(foos.len(), 1);
    assert_eq!(foos[0].val, "hello");
    Ok(())
}

#[driver_test(id(ID), requires(sql))]
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
            Err(toasty::Error::transaction_rollback())
        })
        .await;

    assert!(result.is_err());

    let foos: Vec<Foo> = Foo::all().collect(&db).await?;
    assert_eq!(foos.len(), 0);
    Ok(())
}

#[driver_test(id(ID))]
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

    let err = result.unwrap_err();

    if !t.capability().sql {
        assert!(err.is_unsupported_feature());
        return Ok(());
    }

    assert!(err.is_transaction_timeout());
    let foos: Vec<Foo> = Foo::all().collect(&db).await?;
    assert_eq!(foos.len(), 0);
    Ok(())
}

/// Tests the `Transaction::drop()` rollback path.
///
/// `timeout_rollback` cancels the *user callback* via `TransactionBuilder::timeout`,
/// then `exec()` calls explicit `rollback()`. This test instead drops the entire
/// `exec()` future from outside, exercising the `Drop` impl path.
#[driver_test(id(ID), requires(sql))]
pub async fn cancel_via_drop_rolls_back(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        val: String,
    }

    let db = t.setup_db(models!(Foo)).await;

    // Timeout applied from outside exec(). When it fires, exec() is dropped, which
    // drops Transaction, triggering the Drop impl's rollback — not the explicit
    // rollback in exec().
    let result = tokio::time::timeout(
        Duration::from_millis(10),
        db.transaction(async |tx| {
            Foo::create().val("hello").exec(tx).await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok::<(), toasty::Error>(())
        }),
    )
    .await;

    // Outer tokio timeout yields Elapsed, not a Toasty error.
    assert!(result.is_err());

    let foos: Vec<Foo> = Foo::all().collect(&db).await?;
    assert_eq!(foos.len(), 0);
    Ok(())
}

/// After a future-drop cancellation, the `Db` and its connection pool must
/// still be fully functional.
#[driver_test(id(ID), requires(sql))]
pub async fn db_usable_after_cancel(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        val: String,
    }

    let db = t.setup_db(models!(Foo)).await;

    // Cancel a transaction mid-flight via external drop.
    let _ = tokio::time::timeout(
        Duration::from_millis(10),
        db.transaction(async |tx| {
            Foo::create().val("cancelled").exec(tx).await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok::<(), toasty::Error>(())
        }),
    )
    .await;

    // The Db must still be fully operational: pool connection was released
    // and the bg task recovered.
    db.transaction(async |tx| {
        Foo::create().val("committed").exec(tx).await?;
        Ok::<(), toasty::Error>(())
    })
    .await?;

    let foos: Vec<Foo> = Foo::all().collect(&db).await?;
    assert_eq!(foos.len(), 1);
    assert_eq!(foos[0].val, "committed");
    Ok(())
}

#[driver_test(id(ID))]
pub async fn nested_commit(t: &mut Test) -> Result<()> {
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
            Foo::create().val("outer").exec(tx).await?;

            tx.transaction(async |inner| {
                Foo::create().val("inner").exec(inner).await?;
                Ok::<(), toasty::Error>(())
            })
            .await?;

            Ok::<(), toasty::Error>(())
        })
        .await;

    if !t.capability().sql {
        assert!(result.unwrap_err().is_unsupported_feature());
        return Ok(());
    }

    result?;
    let mut foos: Vec<Foo> = Foo::all().collect(&db).await?;
    foos.sort_by(|a, b| a.val.cmp(&b.val));
    assert_eq!(foos.len(), 2);
    assert_eq!(foos[0].val, "inner");
    assert_eq!(foos[1].val, "outer");
    Ok(())
}

#[driver_test(id(ID))]
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

            Err(toasty::Error::transaction_rollback())
        })
        .await;

    assert!(result.is_err());

    if !t.capability().sql {
        return Ok(());
    }

    let foos: Vec<Foo> = Foo::all().collect(&db).await?;
    assert_eq!(foos.len(), 0);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn nested_rollback(t: &mut Test) -> Result<()> {
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
            Foo::create().val("outer").exec(tx).await?;

            let inner_result: Result<()> = tx
                .transaction(async |inner| {
                    Foo::create().val("inner").exec(inner).await?;
                    Err(toasty::Error::transaction_rollback())
                })
                .await;

            assert!(inner_result.is_err());

            Ok::<(), toasty::Error>(())
        })
        .await;

    if !t.capability().sql {
        assert!(result.unwrap_err().is_unsupported_feature());
        return Ok(());
    }

    result?;
    let foos: Vec<Foo> = Foo::all().collect(&db).await?;
    assert_eq!(foos.len(), 1);
    assert_eq!(foos[0].val, "outer");
    Ok(())
}
