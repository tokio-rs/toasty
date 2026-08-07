use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,

        #[has_many]
        posts: toasty::Deferred<Vec<Post>>,

        #[has_many]
        comments: toasty::Deferred<Vec<Comment>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,

        title: String,

        #[index]
        user_id: uuid::Uuid,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
    }

    #[derive(Debug, toasty::Model)]
    struct Comment {
        #[key]
        #[auto]
        id: uuid::Uuid,

        text: String,

        #[index]
        user_id: uuid::Uuid,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Post, Comment)).await
    }
}
