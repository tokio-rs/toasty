use crate::prelude::*;

use toasty_core::driver::{
    Operation,
    operation::{Transaction, TransactionMode},
};

// `TransactionBuilder::mode(Immediate)` reaches the driver as
// `Transaction::Start { mode: Immediate, .. }` and the transaction
// commits normally. Gated on `transaction_lock_mode` so it only runs
// against drivers that honor the mode (SQLite today).
#[driver_test(
    id(ID),
    requires(transaction_lock_mode),
    scenario(crate::scenarios::two_models)
)]
pub async fn mode_immediate_reaches_driver_and_commits(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    t.log().clear();

    let mut tx = db
        .transaction_builder()
        .mode(TransactionMode::Immediate)
        .begin()
        .await?;
    User::create().name("Alice").exec(&mut tx).await?;
    tx.commit().await?;

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            mode: TransactionMode::Immediate,
            ..
        })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_));
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Commit)
    );
    assert!(t.log().is_empty());

    let users = User::all().exec(&mut db).await?;
    assert_eq!(users.len(), 1);

    Ok(())
}

// Same shape for `Exclusive`. Acquires an exclusive lock for the
// transaction's lifetime — for a single-connection test this is
// indistinguishable from a successful commit, which is what we assert.
#[driver_test(
    id(ID),
    requires(transaction_lock_mode),
    scenario(crate::scenarios::two_models)
)]
pub async fn mode_exclusive_reaches_driver_and_commits(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    t.log().clear();

    let mut tx = db
        .transaction_builder()
        .mode(TransactionMode::Exclusive)
        .begin()
        .await?;
    User::create().name("Bob").exec(&mut tx).await?;
    tx.commit().await?;

    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Start {
            mode: TransactionMode::Exclusive,
            ..
        })
    );
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_));
    assert_struct!(
        t.log().pop_op(),
        Operation::Transaction(Transaction::Commit)
    );
    assert!(t.log().is_empty());

    Ok(())
}

// Drivers without `transaction_lock_mode` must reject non-`Default`
// modes with `unsupported_feature` rather than silently degrading.
// Runs on every SQL driver; SQLite is excluded because it supports the
// mode.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn mode_immediate_rejected_when_unsupported(t: &mut Test) -> Result<()> {
    if t.capability().transaction_lock_mode {
        return Ok(());
    }

    let mut db = setup(t).await;

    let err = match db
        .transaction_builder()
        .mode(TransactionMode::Immediate)
        .begin()
        .await
    {
        Err(e) => e,
        Ok(_) => panic!("driver must reject Immediate when transaction_lock_mode is false"),
    };
    assert!(
        err.is_unsupported_feature(),
        "expected is_unsupported_feature(), got: {err}"
    );

    Ok(())
}
