use crate::prelude::*;

scenario! {
    //! A scalar-terminal `via` declared **without** `Deferred`, so the field is
    //! an eager relation edge: it auto-loads whenever a `User` is queried, with
    //! no explicit `.include()`. Every other via scenario wraps the field in
    //! `Deferred`, leaving the `ViaManyField for Vec<E>` (`DEFERRED = false`)
    //! impl — and via auto-loading — otherwise unexercised.

    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        tags: toasty::Deferred<Vec<Tag>>,

        // Non-deferred scalar via: an eager edge loaded on every `User` query.
        #[has_many(via = tags.name)]
        tag_names: Vec<String>,
    }

    #[derive(Debug, toasty::Model)]
    struct Tag {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Tag)).await
    }
}
