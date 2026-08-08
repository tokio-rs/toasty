use crate::prelude::*;

scenario! {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,

        #[has_many(pair = author)]
        authored_articles: toasty::Deferred<Vec<Article>>,

        #[has_many(pair = reviewer)]
        reviewed_articles: toasty::Deferred<Vec<Article>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Article {
        #[key]
        #[auto]
        id: uuid::Uuid,

        title: String,

        #[index]
        author_id: uuid::Uuid,

        #[index]
        reviewer_id: uuid::Uuid,

        #[belongs_to(key = author_id, references = id)]
        author: toasty::Deferred<User>,

        #[belongs_to(key = reviewer_id, references = id)]
        reviewer: toasty::Deferred<User>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Article)).await
    }
}
