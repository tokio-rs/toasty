use crate::prelude::*;

fn assert_sync_send<T: Send>(val: T) -> T {
    val
}

fn assert_send_sync<T: Send + Sync>() {}

#[driver_test(id(ID), scenario(crate::scenarios::user_unique_email))]
pub async fn ensure_types_sync_send(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let res = assert_sync_send(
        User::filter_by_email("hello@example.com")
            .first()
            .exec(&mut db),
    )
    .await?;

    if let Some(user) = res {
        assert_eq!(user.email, "hello@example.com");
    }

    // Ensure Assignment types are Send + Sync (regression test)
    assert_send_sync::<toasty::stmt::Assignment<String>>();
    assert_send_sync::<toasty::stmt::Assignment<i64>>();
    assert_send_sync::<toasty::stmt::Assignment<toasty::stmt::List<String>>>();

    Ok(())
}
