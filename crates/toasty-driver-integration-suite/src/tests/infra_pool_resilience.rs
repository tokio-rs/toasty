use crate::Fault;
use crate::prelude::*;

/// After a connection is lost, the next call must succeed against a
/// fresh connection — that is the issue from #678. Pinning
/// `max_pool_size = 1` makes eviction observable: the dead slot is the
/// only slot, so the post-failure success proves the pool re-opened.
#[driver_test(
    id(ID),
    requires(test_connection_pool),
    scenario(crate::scenarios::user_with_age)
)]
pub async fn pool_recovers_after_connection_lost(t: &mut Test) -> Result<()> {
    let mut db = t
        .setup_db_with(models!(User), |b| {
            b.max_pool_size(1);
        })
        .await;

    toasty::create!(User {
        name: "alice",
        age: 30
    })
    .exec(&mut db)
    .await?;

    t.inject_fault(Fault::ConnectionLost);

    let err = toasty::create!(User {
        name: "bob",
        age: 30
    })
    .exec(&mut db)
    .await
    .unwrap_err();
    assert!(
        err.is_connection_lost(),
        "expected connection_lost, got {err}",
    );

    toasty::create!(User {
        name: "carol",
        age: 30
    })
    .exec(&mut db)
    .await?;

    let names = User::all()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|u| u.name)
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"alice".to_string()));
    assert!(names.contains(&"carol".to_string()));

    Ok(())
}
