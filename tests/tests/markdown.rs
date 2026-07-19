use std::{fs, path::Path};

use toasty::stmt::Page as ResultPage;
use toasty_driver_markdown::{Markdown, Table};

#[derive(Debug, toasty::Model)]
#[table = "posts"]
struct Post {
    #[key]
    slug: String,
    title: String,
    published: bool,
    score: i64,
    tags: Vec<String>,
    body: String,
}

#[derive(Debug, toasty::Model)]
#[table = "pages"]
struct Page {
    #[key]
    slug: String,
    heading: String,
    markdown: String,
}

#[derive(Debug, toasty::Model)]
#[table = "notes"]
struct Note {
    #[key]
    id: String,
    text: String,
}

fn write(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn post(title: &str, published: bool, score: i64, tags: &[&str], body: &str) -> String {
    let tags = tags
        .iter()
        .map(|tag| format!("  - {tag}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "---\ntitle: {title}\npublished: {published}\nscore: {score}\ntags:\n{tags}\n---\n{body}"
    )
}

#[tokio::test]
async fn loads_conventions_and_executes_memory_queries() {
    let content = tempfile::tempdir().unwrap();
    write(
        content.path().join("posts/rust-guide.md"),
        &post("Rust Guide", true, 10, &["rust", "guide"], "# Rust Guide\n"),
    );
    write(
        content.path().join("posts/rust-news.md"),
        &post("RUST News", true, 30, &["rust", "news"], "# News\n"),
    );
    write(
        content.path().join("posts/draft.md"),
        &post("Rust Draft", false, 50, &["draft"], "# Draft\n"),
    );

    let url = format!("markdown:{}", content.path().display());
    let mut db = toasty::Db::builder()
        .models(toasty::models!(Post))
        .connect(&url)
        .await
        .unwrap();

    assert!(!db.capability().data_mutations);

    let guide = Post::get_by_slug(&mut db, "rust-guide").await.unwrap();
    assert_eq!(guide.title, "Rust Guide");
    assert_eq!(guide.tags, ["rust", "guide"]);
    assert_eq!(guide.body, "# Rust Guide\n");

    let selected: Vec<Post> = Post::filter(
        Post::fields()
            .published()
            .eq(true)
            .and(Post::fields().title().ilike("%rust%".to_string()))
            .and(Post::fields().score().between(10_i64, 40_i64))
            .and(Post::fields().tags().intersects(vec!["rust".to_string()])),
    )
    .order_by(Post::fields().score().desc())
    .limit(1)
    .exec(&mut db)
    .await
    .unwrap();

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].slug, "rust-news");

    let first: ResultPage<Post> = Post::all()
        .order_by(Post::fields().score().asc())
        .paginate(2)
        .exec(&mut db)
        .await
        .unwrap();
    assert_eq!(
        first
            .iter()
            .map(|post| post.slug.as_str())
            .collect::<Vec<_>>(),
        ["rust-guide", "rust-news"]
    );
    let second = first.next(&mut db).await.unwrap().unwrap();
    assert_eq!(second[0].slug, "draft");
    assert!(second.has_prev());
    let previous = second.prev(&mut db).await.unwrap().unwrap();
    assert_eq!(
        previous
            .iter()
            .map(|post| post.slug.as_str())
            .collect::<Vec<_>>(),
        ["rust-guide", "rust-news"]
    );
}

#[tokio::test]
async fn keeps_an_explicit_front_matter_key() {
    let content = tempfile::tempdir().unwrap();
    write(
        content.path().join("posts/filename.md"),
        "---\nslug: canonical\ntitle: Canonical\npublished: true\nscore: 1\ntags: []\n---\nbody",
    );

    let mut db = toasty::Db::builder()
        .models(toasty::models!(Post))
        .build(Markdown::new(content.path()))
        .await
        .unwrap();

    let loaded = Post::get_by_slug(&mut db, "canonical").await.unwrap();
    assert_eq!(loaded.body, "body");
    assert!(Post::get_by_slug(&mut db, "filename").await.is_err());
}

#[tokio::test]
async fn supports_configured_recursive_mappings() {
    let content = tempfile::tempdir().unwrap();
    write(
        content.path().join("articles/guide/intro.md"),
        "---\ntitle: Introduction\n---\nWelcome.\n",
    );

    let driver = Markdown::builder(content.path())
        .table(
            "pages",
            Table::new("articles")
                .column("title", "heading")
                .body_column("markdown")
                .key_from_relative_path("slug")
                .recursive(true),
        )
        .strict(true)
        .build();
    let mut db = toasty::Db::builder()
        .models(toasty::models!(Page))
        .build(driver)
        .await
        .unwrap();

    let page = Page::get_by_slug(&mut db, "guide/intro").await.unwrap();
    assert_eq!(page.heading, "Introduction");
    assert_eq!(page.markdown, "Welcome.\n");
}

#[tokio::test]
async fn snapshot_is_stable_and_mutations_are_rejected() {
    let content = tempfile::tempdir().unwrap();
    let path = content.path().join("posts/stable.md");
    let original = post("Original", true, 1, &["stable"], "Original body\n");
    write(&path, &original);

    let mut db = toasty::Db::builder()
        .models(toasty::models!(Post))
        .build(Markdown::new(content.path()))
        .await
        .unwrap();

    let changed = post("Changed", false, 2, &["changed"], "Changed body\n");
    write(&path, &changed);
    let loaded = Post::get_by_slug(&mut db, "stable").await.unwrap();
    assert_eq!(loaded.title, "Original");

    let create_error = Post::create()
        .slug("new")
        .title("New")
        .published(false)
        .score(0)
        .tags(Vec::<String>::new())
        .body("Draft")
        .exec(&mut db)
        .await
        .unwrap_err();
    assert!(create_error.is_unsupported_feature());

    let mut loaded = loaded;
    let update_error = loaded
        .update()
        .title("Updated")
        .exec(&mut db)
        .await
        .unwrap_err();
    assert!(update_error.is_unsupported_feature());

    let loaded = Post::get_by_slug(&mut db, "stable").await.unwrap();
    let delete_error = loaded.delete().exec(&mut db).await.unwrap_err();
    assert!(delete_error.is_unsupported_feature());
    assert_eq!(fs::read_to_string(path).unwrap(), changed);
}

#[tokio::test]
async fn rejects_invalid_content_and_configuration_during_build() {
    let duplicate = tempfile::tempdir().unwrap();
    for name in ["one", "two"] {
        write(
            duplicate.path().join(format!("posts/{name}.md")),
            "---\nslug: duplicate\ntitle: Duplicate\npublished: true\nscore: 1\ntags: []\n---\n",
        );
    }
    let error = toasty::Db::builder()
        .models(toasty::models!(Post))
        .build(Markdown::new(duplicate.path()))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("duplicate value"));

    let invalid_type = tempfile::tempdir().unwrap();
    write(
        invalid_type.path().join("posts/wrong.md"),
        "---\ntitle: Wrong\npublished: true\nscore: not-a-number\ntags: []\n---\n",
    );
    let error = toasty::Db::builder()
        .models(toasty::models!(Post))
        .build(Markdown::new(invalid_type.path()))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("score"));

    let invalid_mapping = tempfile::tempdir().unwrap();
    fs::create_dir(invalid_mapping.path().join("posts")).unwrap();
    let driver = Markdown::builder(invalid_mapping.path())
        .table(
            "posts",
            Table::new("posts")
                .column("one", "title")
                .column("two", "title"),
        )
        .build();
    let error = toasty::Db::builder()
        .models(toasty::models!(Post))
        .build(driver)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("more than once"));

    let escaping = Markdown::builder(invalid_mapping.path())
        .table("posts", Table::new("../posts"))
        .build();
    let error = toasty::Db::builder()
        .models(toasty::models!(Post))
        .build(escaping)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("relative path"));
}

#[tokio::test]
async fn reusable_memory_driver_reads_typed_rows() {
    let driver = toasty_driver_memory::Memory::builder()
        .table(
            "notes",
            [toasty_core::stmt::ValueRecord::from_vec(vec![
                "one".into(),
                "hello".into(),
            ])],
        )
        .build();
    let mut db = toasty::Db::builder()
        .models(toasty::models!(Note))
        .build(driver)
        .await
        .unwrap();

    let note = Note::get_by_id(&mut db, "one").await.unwrap();
    assert_eq!(note.text, "hello");
}
