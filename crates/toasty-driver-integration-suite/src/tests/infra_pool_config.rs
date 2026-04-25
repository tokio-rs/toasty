use std::time::Duration;

use crate::prelude::*;

#[driver_test(
    id(ID),
    requires(test_connection_pool),
    scenario(crate::scenarios::user_with_age)
)]
pub async fn max_pool_size_is_applied(t: &mut Test) -> Result<()> {
    let db = t
        .setup_db_with(models!(User), |b| {
            b.max_pool_size(7);
        })
        .await;

    assert_eq!(db.pool().status().max_size, 7);

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::user_with_age))]
pub async fn pool_timeouts_are_accepted(t: &mut Test) -> Result<()> {
    // The values are not directly observable through the public API; this
    // test confirms the builder accepts them and a connection can still be
    // acquired.
    t.setup_db_with(models!(User), |b| {
        b.pool_wait_timeout(Some(Duration::from_millis(500)));
        b.pool_create_timeout(Some(Duration::from_secs(2)));
    })
    .await;

    Ok(())
}
