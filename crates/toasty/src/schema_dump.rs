//! Schema extraction support for the standalone `toasty` CLI.
//!
//! `toasty migrate generate` compiles the user's package and runs the
//! resulting artifact with `TOASTY_DUMP_SCHEMA=<flavor>` set. The
//! constructor in this module fires before the user's `main` (or during
//! `dlopen` for lib-only packages built as a `cdylib`), collects every model
//! registered by `#[derive(Model)]`, lowers the schema for the requested
//! flavor, writes it to stdout as JSON, and exits. When the environment
//! variable is not set, the constructor returns immediately.
//!
//! The constructor only exists in builds with `debug_assertions` enabled;
//! release artifacts carry no schema-dump machinery.

use toasty_core::{driver::Capability, schema::db};

/// Environment variable that triggers the schema dump. Its value names the
/// flavor to lower the schema for (e.g. `sqlite`, `postgresql`, `mysql`).
pub const DUMP_SCHEMA_ENV: &str = "TOASTY_DUMP_SCHEMA";

/// Version of the JSON envelope written by the dump constructor.
pub const SCHEMA_DUMP_VERSION: u32 = 1;

/// The JSON envelope exchanged between the schema-dump constructor and the
/// `toasty` CLI.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SchemaDump {
    /// Envelope format version; bumped when the layout changes.
    pub version: u32,

    /// The database-level schema lowered for the requested flavor.
    pub schema: db::Schema,
}

/// Maps a flavor name (as passed in [`DUMP_SCHEMA_ENV`]) to the capability
/// used to lower the schema.
pub fn capability_for_flavor(flavor: &str) -> Option<&'static Capability> {
    match flavor {
        "sqlite" => Some(&Capability::SQLITE),
        "postgresql" | "postgres" => Some(&Capability::POSTGRESQL),
        "mysql" => Some(&Capability::MYSQL),
        "turso" => Some(&Capability::TURSO),
        _ => None,
    }
}

/// Flavor names accepted by [`capability_for_flavor`], for error messages.
pub const FLAVOR_NAMES: &[&str] = &["sqlite", "postgresql", "mysql", "turso"];

#[cfg(debug_assertions)]
mod dump {
    use super::*;
    use crate::schema::DiscoverItem;
    use toasty_core::schema::app::ModelSet;

    #[linktime::ctor(unsafe)]
    fn __toasty_maybe_dump_schema() {
        let Some(flavor) = std::env::var_os(DUMP_SCHEMA_ENV) else {
            return;
        };

        // Once the variable is set, never fall through to the user's `main`:
        // the process exists only to dump the schema.
        match dump_schema(&flavor.to_string_lossy()) {
            Ok(json) => {
                println!("{json}");
                std::process::exit(0);
            }
            Err(message) => {
                eprintln!("error: {message}");
                std::process::exit(1);
            }
        }
    }

    fn dump_schema(flavor: &str) -> Result<String, String> {
        let capability = capability_for_flavor(flavor).ok_or_else(|| {
            format!(
                "`{DUMP_SCHEMA_ENV}={flavor}` names an unknown flavor; expected one of: {}",
                FLAVOR_NAMES.join(", ")
            )
        })?;

        // Collect every model registered by `#[derive(Model)]`, regardless of
        // which crate it came from. Sort by name so the resulting schema (and
        // the snapshot files derived from it) is stable across rebuilds;
        // `inventory` iteration order depends on link order.
        let mut model_set = ModelSet::new();
        for item in crate::schema::inventory::iter::<DiscoverItem> {
            item.add_to(&mut model_set);
        }

        let mut models: Vec<_> = model_set.into_iter().collect();
        models.sort_by(|a, b| a.name().parts.cmp(&b.name().parts));

        let app_schema = toasty_core::schema::app::Schema::from_macro(models)
            .map_err(|err| format!("failed to build schema from registered models: {err}"))?;

        let schema = toasty_core::schema::Builder::new()
            .build(app_schema, capability)
            .map_err(|err| format!("failed to lower schema for flavor `{flavor}`: {err}"))?;

        serde_json::to_string(&SchemaDump {
            version: SCHEMA_DUMP_VERSION,
            schema: schema.db,
        })
        .map_err(|err| format!("failed to serialize schema: {err}"))
    }
}
