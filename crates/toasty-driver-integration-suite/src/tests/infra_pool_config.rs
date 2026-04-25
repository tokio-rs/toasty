use crate::prelude::*;

#[driver_test(id(ID), requires(test_connection_pool))]
pub async fn max_pool_size_is_applied(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
    }

    let db = t
        .setup_db_with(models!(Item), |b| {
            b.max_pool_size(7);
        })
        .await;

    assert_eq!(db.pool().status().max_size, 7);

    Ok(())
}
