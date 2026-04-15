#![cfg(feature = "postgresql")]

use std::sync::Arc;
use std::time::Duration;
use toasty::Db;
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
toasty_driver_integration_suite::generate_driver_tests!(PostgreSqlSetup::new(), bigdecimal_implemented: false);

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
    let conn = toasty_core::driver::Driver::connect(&driver)
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

#[tokio::test]
async fn pool_recovers_db_crash() {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: i64,
    }

    let url = std::env::var("TOASTY_TEST_POSTGRES_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());
    let (admin_client, connection) = tokio_postgres::connect(&url, NoTls)
        .await
        .expect("failed to connect as admin");
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {e}");
        }
    });

    // XXX: We're tagging this pool's connections so we can identifiy them for
    // `pg_terminate_backend`. This allows us to simulate a connection closure
    // without affecting other tests.
    let app_name = "pool_reconnect_test";
    let mut tagged_url = url::Url::parse(&url).expect("failed to parse URL");
    tagged_url
        .query_pairs_mut()
        .append_pair("application_name", app_name);
    let driver = PostgreSQL::new(tagged_url.as_str()).expect("driver creation failed");
    let mut db = Db::builder()
        .models(toasty::models!(User))
        .build(driver)
        .await
        .expect("Db build failed");

    db.push_schema().await.unwrap();

    // Simulate connection closure by killing any backend with our app name
    admin_client
        .execute(
            "SELECT pg_terminate_backend(pid) \
             FROM pg_stat_activity \
             WHERE application_name = $1",
            &[&app_name],
        )
        .await
        .expect("pg_terminate_backend failed");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // The connection gets closed when the backend gets terminated
    assert!(User::filter_by_id(1)
        .exec(&mut db)
        .await
        .is_err_and(|err| err.to_string().eq("connection closed")));

    // After observing a broken connection the pool should discard it and
    // open a fresh one.
    assert!(
        User::filter_by_id(1).exec(&mut db).await.is_ok(),
        "pool did not recover: broken connection was recycled instead of replaced"
    );
}
