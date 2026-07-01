//! quickstart-blog: the smallest believable Toasty app — authors and the posts they write.
//!
//! Run it cold with `cargo run -p example-quickstart-blog` to watch one full pass of
//! define → create → query → update → delete, narrated line by line. Every Toasty idea
//! here is one you reach for in the first hour: models, keys, a unique constraint, a
//! `has_many` / `belongs_to` relationship, and the three query terminals.
//!
//! By default it runs against an in-memory SQLite database, so it needs nothing
//! installed. Point `TOASTY_CONNECTION_URL` at another SQL backend to run it elsewhere.

// A model is a plain Rust struct. `#[derive(toasty::Model)]` reads the fields and
// attributes below to infer the table schema *and* generate every query/create/update
// method used in `main` — you never hand-write SQL or wire up a query builder yourself.
#[derive(Debug, toasty::Model)]
struct Author {
    // `#[key]` marks the primary key; `#[auto]` fills it in on insert. For a `Uuid`
    // that means a time-ordered UUID v7, so you never pass an `id` when creating.
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,

    // `#[unique]` enforces "no two authors share an email" at the database level and is
    // what makes `Author::get_by_email` / `Author::filter_by_email` exist. The attribute
    // drives the API: no `#[unique]`, no by-email lookups.
    #[unique]
    email: String,

    // `has_many` adds no column to `authors` — the link lives on the child's foreign key
    // (`Post::author_id`). `Deferred` means "not loaded until you ask for it"; call
    // `author.posts()` to run that query.
    #[has_many]
    posts: toasty::Deferred<Vec<Post>>,
}

#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: uuid::Uuid,

    title: String,

    // The foreign key back to the author. `#[index]` keeps "find this author's posts"
    // fast instead of scanning the whole table.
    #[index]
    author_id: uuid::Uuid,

    // `belongs_to` is the other side of `Author::posts`. It spells out how the columns
    // line up: this post's `author_id` references an author's `id`.
    #[belongs_to(key = author_id, references = id)]
    author: toasty::Deferred<Author>,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    // The URL scheme selects the driver. `sqlite::memory:` is a throwaway in-process
    // database — ideal for examples and tests because it needs no external service.
    let url =
        std::env::var("TOASTY_CONNECTION_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());

    let mut db = toasty::Db::builder()
        // `models!` discovers every `#[derive(Model)]` in this crate, so you don't list
        // them by hand.
        .models(toasty::models!(crate::*))
        .connect(&url)
        .await?;

    // A fresh database has no tables. `push_schema` creates them straight from the
    // models — the quick path for demos and tests. Real apps evolve a persistent schema
    // with migrations instead (see the `service-ops` example).
    db.push_schema().await?;
    println!("connected to {url}; schema created\n");

    // --- create ---------------------------------------------------------------------
    // `create!` returns a builder; nothing touches the database until `.exec(&mut db)`.
    // The returned value is fully populated, including the auto-generated id.
    let mut alice = toasty::create!(Author {
        name: "Alice",
        email: "alice@example.com",
    })
    .exec(&mut db)
    .await?;
    println!(
        "created author {:?}; its id was generated for us: {}",
        alice.name, alice.id
    );

    // `in alice.posts()` scopes the create to the relationship, so `author_id` is set
    // automatically — you never copy a foreign key by hand.
    toasty::create!(in alice.posts() { title: "Hello, Toasty" })
        .exec(&mut db)
        .await?;
    println!(
        "created a post for {} through her posts() relation",
        alice.name
    );

    // Nested creation builds a parent and its children in one statement. `has_many`
    // children go in an array literal `[{ .. }, { .. }]`.
    let bob = toasty::create!(Author {
        name: "Bob",
        email: "bob@example.com",
        posts: [{ title: "First" }, { title: "Second" }],
    })
    .exec(&mut db)
    .await?;
    println!(
        "created author {:?} with 2 posts in a single statement\n",
        bob.name
    );

    // --- query ----------------------------------------------------------------------
    // `get_by_*` runs immediately and takes `&mut db` as its first argument. `get_by_id`
    // exists for every model; `get_by_email` exists only because `email` is `#[unique]`.
    let by_id = Author::get_by_id(&mut db, &bob.id).await?;
    let by_email = Author::get_by_email(&mut db, "bob@example.com").await?;
    println!(
        "looked Bob up by id and by email — same record: {}",
        by_id.id == by_email.id
    );

    // `all()` / `filter_by_*()` build *lazy* queries. The terminal you pick decides the
    // result shape — match it to what you expect:
    let everyone: Vec<Author> = Author::all().exec(&mut db).await?; // zero-or-more
    let anyone = Author::all().first().exec(&mut db).await?; //         zero-or-one (Option)
    let alice_again = Author::filter_by_email("alice@example.com")
        .get(&mut db) //                                              exactly one (else errors)
        .await?;
    println!(
        "all() -> {} authors; first() -> {:?}; get() -> {:?}\n",
        everyone.len(),
        anyone.map(|a| a.name),
        alice_again.name,
    );

    // `#[unique]` is enforced on every backend: a second author with Alice's email is
    // rejected, not silently merged.
    let dup = toasty::create!(Author {
        name: "Imposter",
        email: "alice@example.com",
    })
    .exec(&mut db)
    .await;
    assert!(
        dup.is_err(),
        "duplicate email must be rejected by the unique constraint"
    );
    println!("a duplicate email was correctly rejected\n");

    // --- update ---------------------------------------------------------------------
    // `update!` on a loaded instance writes only the named field — to the database and
    // to `alice` in memory (which is why it must be `mut`).
    toasty::update!(alice {
        name: "Alice Smith"
    })
    .exec(&mut db)
    .await?;
    println!("renamed Alice in place -> {:?}\n", alice.name);

    // --- delete ---------------------------------------------------------------------
    // Scope a delete to a relationship query. Grab one of Bob's posts, then delete it
    // through `bob.posts()`; the delete runs as one statement, no extra round-trip.
    let post = bob
        .posts()
        .first()
        .exec(&mut db)
        .await?
        .expect("Bob has posts");
    bob.posts()
        .filter_by_id(post.id)
        .delete()
        .exec(&mut db)
        .await?;
    let remaining = bob.posts().exec(&mut db).await?;
    println!(
        "deleted the post {:?}; {} of Bob's posts remain",
        post.title,
        remaining.len()
    );

    println!("\ndone — that's the core CRUD + relationship loop in a single pass.");
    Ok(())
}
