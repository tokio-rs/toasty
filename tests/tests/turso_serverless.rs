#![cfg(feature = "turso-serverless")]

//! Runs the driver integration suite against Turso Cloud through the
//! serverless (SQL over HTTP) mode of the Turso driver.
//!
//! Tests lease disposable databases (created through the Turso Platform
//! API with `use_tursodb`) from a small pool: each database serves one
//! test at a time, so no two tests ever share schema state concurrently,
//! while the pool keeps platform API traffic to roughly one create per
//! unit of parallelism instead of one per test. Leftover `toasty-t-*`
//! databases from previous runs are destroyed when a run starts.
//!
//! ```console
//! $ export TOASTY_TEST_TURSO_API_TOKEN=<platform token>
//! $ export TOASTY_TEST_TURSO_ORG=<org slug>
//! $ export TOASTY_TEST_TURSO_GROUP=<group>
//! $ cargo test -p tests --features turso-serverless --test turso_serverless
//! ```

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

static DB_COUNTER: AtomicU32 = AtomicU32::new(0);
/// Databases returned by finished tests, ready for the next lease.
static DB_POOL: std::sync::Mutex<Vec<DisposableDb>> = std::sync::Mutex::new(Vec::new());
/// One bearer token authenticates to every database in the group; minted
/// once per run to keep platform API traffic to a minimum.
static GROUP_TOKEN: OnceLock<String> = OnceLock::new();
/// Destroys leftovers from previous runs, once per process.
static SWEEP: std::sync::Once = std::sync::Once::new();

/// A pool lease: derefs to the database and returns it to the pool on
/// drop — including on panic, since drops run during unwinding.
struct DbLease(Option<DisposableDb>);

impl std::ops::Deref for DbLease {
    type Target = DisposableDb;
    fn deref(&self) -> &DisposableDb {
        self.0
            .as_ref()
            .expect("lease holds a database until dropped")
    }
}

impl Drop for DbLease {
    fn drop(&mut self) {
        if let Some(db) = self.0.take() {
            DB_POOL.lock().unwrap().push(db);
        }
    }
}

fn env(name: &str) -> String {
    std::env::var(name)
        .unwrap_or_else(|_| panic!("set {name} to run the serverless suite against Turso Cloud"))
}

/// Calls the Turso Platform API, retrying transient failures — including
/// rate limiting, which needs a long cool-down — and returns the response
/// body. Runs on a fresh thread so the blocking client works from within
/// the tests' async runtimes.
fn platform_api(method: &str, path: &str, body: Option<serde_json::Value>) -> serde_json::Value {
    let url = format!(
        "https://api.turso.tech/v1/organizations/{}/{path}",
        env("TOASTY_TEST_TURSO_ORG")
    );
    let token = env("TOASTY_TEST_TURSO_API_TOKEN");
    let method = method.to_string();
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let mut last = String::new();
        for delay_ms in [0u64, 500, 2_000, 8_000, 20_000, 45_000] {
            if delay_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
            let mut req = client
                .request(method.parse().unwrap(), &url)
                .bearer_auth(&token);
            if let Some(body) = &body {
                req = req.json(body);
            }
            match req.send().and_then(|resp| resp.error_for_status()) {
                Ok(resp) => {
                    return resp.json().unwrap_or(serde_json::Value::Null);
                }
                Err(err) => last = err.to_string(),
            }
        }
        panic!("{method} {url} failed: {last}");
    })
    .join()
    .expect("platform API call panicked")
}

/// A disposable Turso Cloud database: created through the platform API,
/// leased to one test at a time.
struct DisposableDb {
    url: String,
    token: String,
}

impl DisposableDb {
    /// Leases a database: reuses one returned by a finished test, or
    /// creates a fresh one.
    fn lease() -> DbLease {
        SWEEP.call_once(|| {
            let listing = platform_api("GET", "databases", None);
            for db in listing["databases"].as_array().into_iter().flatten() {
                if let Some(name) = db["Name"].as_str()
                    && name.starts_with("toasty-t-")
                {
                    platform_api("DELETE", &format!("databases/{name}"), None);
                }
            }
        });
        let db = DB_POOL.lock().unwrap().pop().unwrap_or_else(Self::create);
        DbLease(Some(db))
    }

    fn create() -> Self {
        let name = format!(
            "toasty-t-{}-{}",
            std::process::id(),
            DB_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let created = platform_api(
            "POST",
            "databases",
            Some(serde_json::json!({
                "name": name,
                "group": env("TOASTY_TEST_TURSO_GROUP"),
                "use_tursodb": true,
            })),
        );
        let hostname = created["database"]["Hostname"]
            .as_str()
            .unwrap_or_else(|| panic!("database creation returned no hostname: {created}"))
            .to_string();
        let token = GROUP_TOKEN
            .get_or_init(|| {
                let group = env("TOASTY_TEST_TURSO_GROUP");
                platform_api("POST", &format!("groups/{group}/auth/tokens"), None)["jwt"]
                    .as_str()
                    .expect("group token mint returned no jwt")
                    .to_string()
            })
            .clone();
        Self {
            url: format!("turso://{hostname}"),
            token,
        }
    }

    fn driver(&self) -> toasty_driver_turso::Turso {
        toasty_driver_turso::Turso::new(&self.url)
            .expect("failed to create Turso driver")
            .with_auth_token(&self.token)
    }

    /// Connection URL carrying the token, for the `Db::builder().connect()`
    /// entry-point tests.
    fn connect_url(&self) -> String {
        format!("{}?authToken={}", self.url, self.token)
    }
}

struct TursoServerlessSetup {
    db: OnceLock<DbLease>,
}

impl TursoServerlessSetup {
    fn new() -> Self {
        Self {
            db: OnceLock::new(),
        }
    }
}

#[async_trait::async_trait]
impl toasty_driver_integration_suite::Setup for TursoServerlessSetup {
    fn driver(&self) -> Box<dyn toasty_core::driver::Driver> {
        Box::new(self.db.get_or_init(DisposableDb::lease).driver())
    }

    /// Drops the test's (uniquely prefixed) tables so the database goes
    /// back to the pool lean. Best effort: the lease guarantees nobody
    /// else is using the database, and a missed drop only leaves an
    /// unreferenced table behind.
    async fn delete_table(&self, name: &str) {
        let Some(db) = self.db.get() else { return };
        let Ok(handle) = turso_serverless::Builder::new_remote(db.url.clone())
            .with_auth_token(db.token.clone())
            .build()
            .await
        else {
            return;
        };
        if let Ok(conn) = handle.connect() {
            let _ = conn
                .execute(&format!("DROP TABLE IF EXISTS \"{name}\""), ())
                .await;
        }
    }
}

// Generate all driver tests. Capability flags match the embedded Turso
// setup in `turso.rs`: the cloud runs the same engine, only the transport
// differs.
toasty_driver_integration_suite::generate_driver_tests!(
    TursoServerlessSetup::new(),
    native_decimal: false,
    bigdecimal_implemented: false,
    decimal_arbitrary_precision: false,
    native_timestamp: false,
    native_date: false,
    native_time: false,
    native_datetime: false,
    native_cidr: false,
    native_inet: false,
    native_macaddr: false,
    native_macaddr8: false,
    native_array: false,
    native_ilike: false,
    native_json: false,
    native_jsonb: false,
    native_enum: false,
    unique_list_index: false,
    vec_scalar: true,
    vec_remove: false,
    vec_pop: false,
    vec_remove_at: false,
    test_connection_pool: true,
);

#[derive(Debug, toasty::Model)]
struct MvccCounter {
    #[key]
    id: i64,
    tally: i64,
}

/// Integration-owned MVCC facts: the driver's serverless default runs
/// transactions under `BEGIN CONCURRENT`, and a write-write conflict
/// comes back classified as a retryable serialization failure. The
/// engine's MVCC semantics themselves are turso's conformance suite's
/// job, not ours.
#[tokio::test]
async fn mvcc_conflicts_classify_as_serialization_failures() {
    let disposable = DisposableDb::lease();

    let mut db = toasty::Db::builder()
        .models(toasty::models!(MvccCounter))
        .max_pool_size(4)
        .build(disposable.driver())
        .await
        .unwrap();
    db.push_schema().await.unwrap();
    toasty::create!(MvccCounter { id: 1, tally: 0 })
        .exec(&mut db)
        .await
        .unwrap();

    let mut db_a = db.clone();
    let mut db_b = db.clone();
    let mut tx_a = db_a.transaction().await.unwrap();
    let mut tx_b = db_b.transaction().await.unwrap();
    MvccCounter::filter_by_id(1i64)
        .update()
        .tally(1)
        .exec(&mut tx_a)
        .await
        .unwrap();
    let conflict = async {
        MvccCounter::filter_by_id(1i64)
            .update()
            .tally(2)
            .exec(&mut tx_b)
            .await?;
        tx_b.commit().await
    }
    .await;
    tx_a.commit().await.expect("winning commit must succeed");

    let err = conflict.expect_err("colliding BEGIN CONCURRENT writers must conflict");
    assert!(
        err.is_serialization_failure(),
        "expected serialization failure, got: {err}"
    );
}

#[derive(Debug, toasty::Model)]
struct SmokeItem {
    #[key]
    id: i64,
    name: String,
}

/// The single entry point every backend uses: one connection URL into
/// `Db::builder().connect()`. Scheme routing picks the Turso driver, the
/// host selects serverless mode, and the `authToken` query parameter
/// carries the credential. Everything past the connection is the
/// generated suite's job.
#[tokio::test]
async fn connect_via_url_with_auth_token() {
    let disposable = DisposableDb::lease();

    let mut db = toasty::Db::builder()
        .models(toasty::models!(SmokeItem))
        .connect(&disposable.connect_url())
        .await
        .unwrap();
    db.push_schema().await.unwrap();

    toasty::create!(SmokeItem {
        id: 1,
        name: "hello"
    })
    .exec(&mut db)
    .await
    .unwrap();
    let read = SmokeItem::get_by_id(&mut db, &1).await.unwrap();
    assert_eq!(read.name, "hello");
}
