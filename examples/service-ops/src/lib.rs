//! service-ops: a deployable Toasty service laid out the way a real app ships — shared models
//! and connection setup in a library, consumed by two binaries: an app `server` and a
//! project-local `migrate` CLI. Keeping the models in one library is what lets the migration
//! tool diff against exactly the types the server runs.
//!
//! See `src/bin/server.rs` (the app) and `src/bin/migrate.rs` (the migration CLI).

#[derive(Debug, toasty::Model)]
pub struct Tenant {
    #[key]
    #[auto]
    pub id: uuid::Uuid,

    #[unique]
    pub slug: String,

    pub name: String,

    #[has_many]
    pub api_keys: toasty::Deferred<Vec<ApiKey>>,
}

#[derive(Debug, toasty::Model)]
pub struct ApiKey {
    #[key]
    #[auto]
    pub id: uuid::Uuid,

    #[index]
    pub tenant_id: uuid::Uuid,

    #[belongs_to]
    pub tenant: toasty::Deferred<Tenant>,

    #[unique]
    pub token: String,
}

/// Build the shared `Db`. Centralizing connection and pool configuration here keeps the two
/// binaries in sync.
///
/// The URL comes from `TOASTY_CONNECTION_URL`, defaulting to an in-memory database so the
/// `server` demo runs cold and is re-runnable (it calls `push_schema`, which builds the
/// schema fresh each time). For the migration workflow, point it at a *persistent* database
/// so applied migrations survive between CLI invocations, e.g.:
///   `TOASTY_CONNECTION_URL=sqlite:./service.db cargo run --bin migrate -- migration apply`
pub async fn build_db() -> toasty::Result<toasty::Db> {
    let url =
        std::env::var("TOASTY_CONNECTION_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());

    toasty::Db::builder()
        .models(toasty::models!(crate::*))
        .max_pool_size(32)
        .pool_pre_ping(true) // check a pooled connection is alive before handing it out
        .table_name_prefix("svc_") // namespace tables so several services can share one database
        .connect(&url)
        .await
}
