use crate::prelude::*;

scenario! {
    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        comments: toasty::HasMany<Comment>,

        // User → comments → article
        #[has_many(via = comments.article)]
        commented_articles: toasty::HasMany<Article>,
    }

    #[derive(Debug, toasty::Model)]
    struct Article {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[has_many]
        comments: toasty::HasMany<Comment>,
    }

    #[derive(Debug, toasty::Model)]
    struct Comment {
        #[key]
        #[auto]
        id: ID,

        body: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[index]
        article_id: ID,

        #[belongs_to(key = article_id, references = id)]
        article: toasty::BelongsTo<Article>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Article, Comment)).await
    }
}
