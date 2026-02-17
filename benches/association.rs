use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;
use tests::{models, LoggingDriver, Setup};
use toasty::stmt::Id;

type SetupFactory = Box<dyn Fn() -> Box<dyn Setup>>;

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,
    name: String,
    #[has_many]
    posts: toasty::HasMany<Post>,
    #[has_many]
    comments: toasty::HasMany<Comment>,
}

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct Post {
    #[key]
    #[auto]
    id: Id<Self>,
    title: String,
    #[index]
    user_id: Id<User>,
    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,
}

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct Comment {
    #[key]
    #[auto]
    id: Id<Self>,
    text: String,
    #[index]
    user_id: Id<User>,
    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,
}

async fn setup_test_data(
    db: &toasty::Db,
    num_users: usize,
    posts_per_user: usize,
    comments_per_user: usize,
) {
    for i in 0..num_users {
        let user = User::create()
            .name(format!("User {}", i))
            .exec(db)
            .await
            .unwrap();

        for j in 0..posts_per_user {
            Post::create()
                .title(format!("Post {} for User {}", j, i))
                .user(&user)
                .exec(db)
                .await
                .unwrap();
        }

        for j in 0..comments_per_user {
            Comment::create()
                .text(format!("Comment {} for User {}", j, i))
                .user(&user)
                .exec(db)
                .await
                .unwrap();
        }
    }
}

async fn setup_database_and_data(
    setup: Box<dyn Setup>,
    users: usize,
    posts: usize,
    comments: usize,
) -> toasty::Db {
    let mut builder = models!(User, Post, Comment);
    setup.configure_builder(&mut builder);

    let logging_driver = LoggingDriver::new(setup.driver());
    let db = builder.build(logging_driver).await.unwrap();
    db.push_schema().await.unwrap();

    setup_test_data(&db, users, posts, comments).await;

    db
}

fn association_benchmarks(c: &mut Criterion) {
    let sizes = vec![(50, 10, 10), (100, 20, 20), (200, 25, 25)];

    let database_setups: Vec<(&str, SetupFactory)> = vec![
        #[cfg(feature = "sqlite")]
        (
            "sqlite",
            Box::new(|| Box::new(tests::db::sqlite::SetupSqlite::new()) as Box<dyn Setup>),
        ),
        #[cfg(feature = "postgresql")]
        (
            "postgresql",
            Box::new(|| Box::new(tests::db::postgresql::SetupPostgreSQL::new()) as Box<dyn Setup>),
        ),
        #[cfg(feature = "mysql")]
        (
            "mysql",
            Box::new(|| Box::new(tests::db::mysql::SetupMySQL::new()) as Box<dyn Setup>),
        ),
        #[cfg(feature = "dynamodb")]
        (
            "dynamodb",
            Box::new(|| Box::new(tests::db::dynamodb::SetupDynamoDb::new()) as Box<dyn Setup>),
        ),
    ];

    for (db_name, setup_fn) in database_setups {
        let mut group = c.benchmark_group(format!("association_performance_{}", db_name));
        group.sample_size(10);

        for (users, posts, comments) in &sizes {
            let size_label = format!("{}u_{}p_{}c", users, posts, comments);

            let rt = tokio::runtime::Runtime::new().unwrap();
            let db = rt.block_on(async {
                setup_database_and_data(setup_fn(), *users, *posts, *comments).await
            });

            group.bench_with_input(
                BenchmarkId::new("multiple_has_many", &size_label),
                &size_label,
                |b, _| {
                    b.iter(|| {
                        rt.block_on(async {
                            let users: Vec<User> = User::all()
                                .include(User::FIELDS.posts())
                                .include(User::FIELDS.comments())
                                .collect(&db)
                                .await
                                .unwrap();
                            black_box(users)
                        })
                    });
                },
            );
        }
        group.finish();
    }
}

criterion_group!(benches, association_benchmarks);
criterion_main!(benches);
