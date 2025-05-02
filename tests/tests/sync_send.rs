use tests::*;
use toasty::stmt::Id;

fn assert_sync_send<T: Send>(val: T) -> T {
    val
}

async fn ensure_types_sync_send(s: impl Setup) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[unique]
        email: String,
    }

    let db = s.setup(models!(User)).await;

    let res = assert_sync_send(User::filter_by_email("hello@example.com").first(&db))
        .await
        .unwrap();

    if let Some(user) = res {
        assert_eq!(user.email, "hello@example.com");
    }
}

tests!(ensure_types_sync_send);
