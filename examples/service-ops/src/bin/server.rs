//! service-ops (server binary): how you OPERATE a Toasty service — a pooled `Db`, structured
//! tracing, and a sibling `migrate` binary for schema changes. Models live in `src/lib.rs`.
//!
//! Run it (see each statement Toasty issues):
//!   RUST_LOG=toasty=debug cargo run -p example-service-ops --bin server

use example_service_ops::{ApiKey, Tenant, build_db};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Toasty emits structured tracing events but prints NOTHING until a subscriber is
    // installed. With one, `RUST_LOG=toasty=debug` shows each statement (db.system /
    // db.statement). Parameter VALUES are never logged — log them yourself if you need them.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let mut db = build_db().await?;

    // `push_schema` is the fast path for dev and tests. The `migrate` binary is the production
    // path for evolving a schema that already holds data.
    db.push_schema().await?;

    // Get-or-create so the demo is re-runnable against the persistent database.
    let tenant = match Tenant::get_by_slug(&mut db, "acme").await {
        Ok(tenant) => tenant,
        Err(_) => {
            toasty::create!(Tenant {
                slug: "acme",
                name: "Acme Inc",
                api_keys: [{ token: "key_live_1" }, { token: "key_live_2" }],
            })
            .exec(&mut db)
            .await?
        }
    };

    let keys = tenant.api_keys().exec(&mut db).await?;
    println!("tenant {:?} has {} api key(s)", tenant.name, keys.len());

    // Authenticate a request by its unique token.
    let key = ApiKey::get_by_token(&mut db, "key_live_1").await?;
    println!("token key_live_1 belongs to tenant {}", key.tenant_id);

    Ok(())
}
