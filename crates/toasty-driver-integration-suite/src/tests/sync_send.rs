use crate::prelude::*;

fn assert_sync_send<T: Send>(val: T) -> T {
    val
}

#[driver_test(id(ID))]
pub async fn ensure_types_sync_send(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        email: String,
    }

    let db = t.setup_db(models!(User)).await;

    let res = assert_sync_send(User::filter_by_email("hello@example.com").first(&db)).await?;

    if let Some(user) = res {
        assert_eq!(user.email, "hello@example.com");
    }
    Ok(())
}
