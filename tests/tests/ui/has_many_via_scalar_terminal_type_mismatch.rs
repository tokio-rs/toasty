// A scalar-terminal `via` whose declared element type disagrees with the type
// the path actually reaches must be a compile error, not a runtime load
// failure. Here the path reaches `Article::title` (a `String`) but the field is
// declared `Vec<i64>`.

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: i64,

    name: String,

    #[has_many]
    comments: toasty::Deferred<Vec<Comment>>,

    #[has_many(via = comments.article.title)]
    article_titles: toasty::Deferred<Vec<i64>>,
}

#[derive(Debug, toasty::Model)]
struct Article {
    #[key]
    id: i64,

    title: String,

    #[has_many]
    comments: toasty::Deferred<Vec<Comment>>,
}

#[derive(Debug, toasty::Model)]
struct Comment {
    #[key]
    id: i64,

    user_id: i64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::Deferred<User>,

    article_id: i64,

    #[belongs_to(key = article_id, references = id)]
    article: toasty::Deferred<Article>,
}

fn main() {}
