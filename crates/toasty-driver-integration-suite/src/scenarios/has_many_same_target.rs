use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many(pair = author)]
        authored_articles: toasty::HasMany<Article>,

        #[has_many(pair = reviewer)]
        reviewed_articles: toasty::HasMany<Article>,
    }

    #[derive(Debug, toasty::Model)]
    struct Article {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        author_id: ID,

        #[index]
        reviewer_id: ID,

        #[belongs_to(key = author_id, references = id)]
        author: toasty::BelongsTo<User>,

        #[belongs_to(key = reviewer_id, references = id)]
        reviewer: toasty::BelongsTo<User>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Article)).await
    }
}
