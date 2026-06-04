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
        comments: toasty::Deferred<Vec<Comment>>,

        // User → comments → article
        #[has_many(via = comments.article)]
        commented_articles: toasty::Deferred<Vec<Article>>,

        // User → comments → article → title (scalar terminal)
        #[has_many(via = comments.article.title)]
        commented_article_titles: toasty::Deferred<Vec<String>>,

        // User → comments → body: a 2-step scalar terminal. The terminal field
        // sits directly on the first relation's target, so the relation chain is
        // a single step — the minimal scalar-via walk.
        #[has_many(via = comments.body)]
        comment_bodies: toasty::Deferred<Vec<String>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Article {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[has_many]
        comments: toasty::Deferred<Vec<Comment>>,
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
        user: toasty::Deferred<User>,

        #[index]
        article_id: ID,

        #[belongs_to(key = article_id, references = id)]
        article: toasty::Deferred<Article>,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Article, Comment)).await
    }
}
