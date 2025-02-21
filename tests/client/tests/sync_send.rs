use tests_client::*;

fn assert_sync_send<T: Send>(_: T) {}

async fn ensure_types_sync_send(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            #[unique]
            email: String,
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    assert_sync_send(db::User::filter_by_email("hello@example.com").first(&db));
}

tests!(ensure_types_sync_send);
