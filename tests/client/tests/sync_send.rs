use tests_client::*;
use toasty::stmt::Id;

fn assert_sync_send<T: Send>(_: T) {}

async fn ensure_types_sync_send(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[unique]
        email: String,
    }

    let db = s.setup(models!(User)).await;

    assert_sync_send(User::filter_by_email("hello@example.com").first(&db));
}

tests!(ensure_types_sync_send);
