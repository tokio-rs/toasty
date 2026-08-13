#![cfg(feature = "postgresql")]

use std::sync::Arc;
use toasty_driver_postgresql::PostgreSQL;
use tokio::sync::OnceCell;
use tokio_postgres::NoTls;

struct PostgreSqlSetup {
    client: OnceCell<Arc<tokio_postgres::Client>>,
}

impl PostgreSqlSetup {
    fn new() -> Self {
        Self {
            client: OnceCell::new(),
        }
    }

    async fn get_client(&self) -> &Arc<tokio_postgres::Client> {
        self.client
            .get_or_init(|| async {
                let url = std::env::var("TOASTY_TEST_POSTGRES_URL")
                    .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());

                let (client, connection) = tokio_postgres::connect(&url, NoTls)
                    .await
                    .expect("Failed to connect to PostgreSQL");

                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("connection error: {}", e);
                    }
                });

                Arc::new(client)
            })
            .await
    }
}

#[async_trait::async_trait]
impl toasty_driver_integration_suite::Setup for PostgreSqlSetup {
    fn driver(&self) -> Box<dyn toasty_core::driver::Driver> {
        let url = std::env::var("TOASTY_TEST_POSTGRES_URL")
            .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());
        Box::new(PostgreSQL::new(&url).expect("Failed to create PostgreSQL driver"))
    }

    async fn delete_table(&self, name: &str) {
        let client = self.get_client().await;

        let sql = format!("DROP TABLE IF EXISTS \"{}\" CASCADE", name);
        client
            .execute(&sql, &[])
            .await
            .expect("Failed to drop table");
    }
}

// Generate all driver tests
toasty_driver_integration_suite::generate_driver_tests!(
    PostgreSqlSetup::new(),
    bigdecimal_implemented: false,
    transaction_lock_mode: false,
);

#[tokio::test]
async fn url_encoding() {
    let admin_url = std::env::var("TOASTY_TEST_POSTGRES_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());
    let (admin_client, connection) = tokio_postgres::connect(&admin_url, NoTls)
        .await
        .expect("failed to connect as admin");
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {e}");
        }
    });

    let role = "url_encoding#role";
    let dbname = "url_encoding#db";
    let password = "p@ss#word";

    // Idempotent setup: drop if leftover from a previous run
    admin_client
        .execute(&format!("DROP DATABASE IF EXISTS \"{}\"", dbname), &[])
        .await
        .expect("failed to drop test database");
    admin_client
        .execute(&format!("DROP ROLE IF EXISTS \"{}\"", role), &[])
        .await
        .expect("failed to drop test role");

    admin_client
        .execute(
            &format!(
                "CREATE ROLE \"{}\" WITH LOGIN PASSWORD '{}'",
                role, password
            ),
            &[],
        )
        .await
        .expect("failed to create test role");
    admin_client
        .execute(
            &format!("CREATE DATABASE \"{}\" OWNER \"{}\"", dbname, role),
            &[],
        )
        .await
        .expect("failed to create test database");

    // Parse the admin URL to extract host/port
    let parsed = url::Url::parse(&admin_url).expect("failed to parse admin URL");
    let host = parsed.host_str().unwrap_or("localhost");
    let port = parsed.port().unwrap_or(5432);

    let encoded_role = url::form_urlencoded::byte_serialize(role.as_bytes()).collect::<String>();
    let encoded_password =
        url::form_urlencoded::byte_serialize(password.as_bytes()).collect::<String>();
    let encoded_dbname =
        url::form_urlencoded::byte_serialize(dbname.as_bytes()).collect::<String>();

    let test_url = format!(
        "postgresql://{}:{}@{}:{}/{}",
        encoded_role, encoded_password, host, port, encoded_dbname
    );

    let driver = PostgreSQL::new(&test_url).expect("driver creation failed");
    let conn = toasty_core::driver::Driver::connect(
        &driver,
        &toasty_core::driver::ConnectContext::default(),
    )
    .await
    .expect("connection with percent-encoded URL failed");
    drop(conn);

    // Cleanup
    admin_client
        .execute(&format!("DROP DATABASE IF EXISTS \"{}\"", dbname), &[])
        .await
        .expect("failed to drop test database");
    admin_client
        .execute(&format!("DROP ROLE IF EXISTS \"{}\"", role), &[])
        .await
        .expect("failed to drop test role");
}

/// One `Db` per PostgreSQL schema via libpq's `options` startup
/// parameter (#1077). The pool builds every connection from the same
/// config, so each one starts with the configured `search_path`:
/// `push_schema` creates the tables in that schema, and unqualified
/// names resolve there on every pooled connection.
#[tokio::test]
async fn search_path_via_url_options() {
    #[derive(Debug, toasty::Model)]
    struct SearchPathTodo {
        #[key]
        #[auto]
        id: uuid::Uuid,
        title: String,
    }

    let admin_url = std::env::var("TOASTY_TEST_POSTGRES_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls)
        .await
        .expect("failed to connect as admin");
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {e}");
        }
    });

    let schema = format!("search_path_test_{}", std::process::id());
    admin
        .execute(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"), &[])
        .await
        .expect("failed to drop leftover schema");
    admin
        .execute(&format!("CREATE SCHEMA \"{schema}\""), &[])
        .await
        .expect("failed to create schema");

    let options =
        url::form_urlencoded::byte_serialize(format!("-c search_path={schema}").as_bytes())
            .collect::<String>();
    let sep = if admin_url.contains('?') { '&' } else { '?' };
    let test_url = format!("{admin_url}{sep}options={options}");

    let mut builder = toasty::Db::builder();
    builder.models(toasty::models!(SearchPathTodo));
    let mut db = builder.connect(&test_url).await.expect("connect failed");
    db.push_schema().await.expect("push_schema failed");

    let created_in_schema: bool = admin
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables \
             WHERE table_schema = $1 AND table_name = 'search_path_todos')",
            &[&schema],
        )
        .await
        .expect("table lookup failed")
        .get(0);
    assert!(
        created_in_schema,
        "push_schema did not create the table in schema {schema}"
    );

    // The table exists only in the test schema, so an insert succeeds
    // only on a connection whose search_path includes it. Run enough
    // concurrent inserts to grow the pool past one connection.
    let tasks: Vec<_> = (0..16)
        .map(|i| {
            let mut db = db.clone();
            tokio::spawn(async move {
                toasty::create!(SearchPathTodo {
                    title: format!("todo-{i}"),
                })
                .exec(&mut db)
                .await
                .expect("insert failed")
            })
        })
        .collect();
    let mut todos = Vec::new();
    for task in tasks {
        todos.push(task.await.expect("insert task panicked"));
    }

    let count: i64 = admin
        .query_one(
            &format!("SELECT count(*) FROM \"{schema}\".search_path_todos"),
            &[],
        )
        .await
        .expect("count failed")
        .get(0);
    assert_eq!(count, 16);

    let reloaded = SearchPathTodo::get_by_id(&mut db, &todos[0].id)
        .await
        .expect("get_by_id failed");
    assert_eq!(reloaded.title, todos[0].title);

    admin
        .execute(&format!("DROP SCHEMA \"{schema}\" CASCADE"), &[])
        .await
        .expect("failed to drop schema");
}

/// Connectivity smoke test for Unix-domain sockets (regression for #984).
///
/// The driver must accept libpq-style `?host=/path` URLs so the
/// directory containing the PG socket can be specified — URL syntax
/// cannot encode a filesystem path as the authority. Skipped when
/// `TOASTY_TEST_POSTGRES_UDS_URL` is unset so the default test runs
/// stay green on machines without a socket-accessible PG.
#[tokio::test]
async fn connect_via_unix_socket() {
    let Ok(url) = std::env::var("TOASTY_TEST_POSTGRES_UDS_URL") else {
        eprintln!("TOASTY_TEST_POSTGRES_UDS_URL unset; skipping UDS connectivity test");
        return;
    };

    toasty::Db::builder()
        .connect(&url)
        .await
        .expect("PostgreSQL connection over Unix-domain socket failed");
}
