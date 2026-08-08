//! cms-article-fields: a CMS publishing flow on a single Article — draft it, count views,
//! curate tags, lazily load the body, then publish. Along the way it answers "what can I
//! attach to one field?": custom column names, create/update defaults, auto timestamps, a
//! queryable `Vec<scalar>`, a deferred column, and opaque JSON.
//!
//! Run it cold (`cargo run -p example-cms-article-fields`). In-memory SQLite by default;
//! set `TOASTY_CONNECTION_URL` for another backend.

// A plain serde type. `Json<SeoMeta>` stores it as one opaque JSON column — handy for a
// value with no native column representation, at the cost of not being queryable.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SeoMeta {
    description: String,
    canonical_url: Option<String>,
}

// A fixed set of lifecycle states is better modeled as an embedded enum than a `String`: typos
// become compile errors and `match`es stay exhaustive. With no `#[column(variant)]`, each
// variant is stored in one column as its name in snake_case ("draft", "published", "archived");
// override the stored label with `#[column(variant = "d")]` or an integer if you need to.
#[derive(Debug, PartialEq, toasty::Embed)]
enum ArticleStatus {
    Draft,
    Published,
    Archived,
}

#[derive(Debug, toasty::Model)]
// Custom table name. By default Toasty snake_cases and pluralizes the struct name, so `Article`
// would map to `articles`; we override it to namespace the table for a CMS schema.
#[table = "cms_articles"]
struct Article {
    #[key]
    #[auto]
    id: uuid::Uuid,

    // The Rust field stays `title`; the column is `headline`.
    #[column("headline")]
    title: String,

    // A second auto field that is NOT the key. `uuid(v4)` is a random id (vs the default
    // time-ordered v7), which is what you want for an unguessable share token.
    #[auto(uuid(v4))]
    share_token: uuid::Uuid,

    // `Option<T>` is a nullable column. Left unset on create, it stores NULL / reads None.
    subtitle: Option<String>,

    // `#[default(expr)]` runs at insert time and applies ONLY on create. The expression is any
    // Rust value — a literal, or an enum variant.
    #[default(0)]
    view_count: i64,
    #[default(ArticleStatus::Draft)]
    status: ArticleStatus,

    // `#[default]` sets this once, on create. `#[update]` re-evaluates on every write, so
    // `updated_at` tracks the last change while `created_at` is fixed.
    #[default(jiff::Timestamp::now())]
    created_at: jiff::Timestamp,
    #[update(jiff::Timestamp::now())]
    updated_at: jiff::Timestamp,

    // A queryable scalar collection (one column). Unlike `Json<T>`, you can filter and
    // incrementally mutate it.
    tags: Vec<String>,

    // A large column omitted from the default SELECT; load it on demand with `.include()`.
    body: toasty::Deferred<String>,

    #[column(type = text)]
    seo: toasty::Json<SeoMeta>,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let url =
        std::env::var("TOASTY_CONNECTION_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let mut db = toasty::Db::builder()
        .models(toasty::models!(crate::*))
        .connect(&url)
        .await?;
    db.push_schema().await?;

    // Set only what we want; everything else comes from #[default]/#[auto]/#[update].
    let mut article = toasty::create!(Article {
        title: "Hello, Toasty",
        body: "a long body we usually don't read in list views".to_string(),
        tags: ["rust", "orm"], // array literal — no vec! needed
        seo: toasty::Json(SeoMeta {
            description: "an intro to Toasty".into(),
            canonical_url: None,
        }),
    })
    .exec(&mut db)
    .await?;

    assert_eq!(article.view_count, 0); // #[default(0)] fired on create
    assert_eq!(article.status, ArticleStatus::Draft); // #[default(..)] value
    assert!(article.subtitle.is_none()); // omitted Option -> None / SQL NULL
    println!(
        "drafted {:?}: status={:?}, views={}, share_token={}",
        article.title, article.status, article.view_count, article.share_token
    );

    // Register a page view. A relative numeric update folds read+write into ONE atomic
    // statement, so concurrent viewers can't clobber each other (a Rust-side
    // read-modify-write would race).
    toasty::update!(article { view_count.increment() })
        .exec(&mut db)
        .await?;
    println!("after a page view: view_count={}", article.view_count);

    // Incrementally mutate the tag collection. push/extend/clear/set are portable;
    // pop/remove/remove_at are PostgreSQL-only (see postgres-directory).
    toasty::update!(article { tags.push("published") })
        .exec(&mut db)
        .await?;
    println!("tags: {:?}", article.tags);

    // A deferred column is unloaded after a normal query — calling `.get()` on it panics.
    let listed = Article::filter_by_id(article.id).get(&mut db).await?;
    assert!(listed.body.is_unloaded());
    // `.include()` adds it to the SAME query (no extra round-trip); then `.get()` is sync.
    let full = Article::filter_by_id(article.id)
        .include(Article::fields().body())
        .get(&mut db)
        .await?;
    println!("loaded the body on demand: {} bytes", full.body.get().len());

    // Fill in the previously-null subtitle, then publish. Setting `status` explicitly
    // overrides its default; `updated_at` advances while `created_at` stays put.
    toasty::update!(article {
        subtitle: "a gentle introduction"
    })
    .exec(&mut db)
    .await?;
    toasty::update!(article {
        status: ArticleStatus::Published
    })
    .exec(&mut db)
    .await?;
    println!(
        "published: status={:?}, subtitle={:?}, created==updated? {}",
        article.status,
        article.subtitle,
        article.created_at == article.updated_at
    );

    Ok(())
}
