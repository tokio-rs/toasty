//! Schema extraction support for the standalone `toasty` CLI.
//!
//! The CLI builds the user's package and runs the artifact with
//! `TOASTY_DUMP_SCHEMA=<flavor>` set. The constructor below then fires before
//! `main` (or during `dlopen` for a cdylib), collects every model registered
//! by `#[derive(Model)]`, lowers the schema for that flavor, writes it to
//! stdout as JSON, and exits. It only exists in `debug_assertions` builds.

use toasty_core::{driver::Capability, schema::db};

/// Environment variable that triggers the schema dump. Its value names the
/// flavor to lower the schema for (e.g. `sqlite`, `postgresql`, `mysql`).
pub const DUMP_SCHEMA_ENV: &str = "TOASTY_DUMP_SCHEMA";

/// Environment variable carrying the table name prefix to lower the schema
/// with. The constructor runs before any user code, so the prefix set on
/// [`Builder::table_name_prefix`](crate::db::Builder::table_name_prefix) is
/// invisible to it; the CLI passes `migration.table_name_prefix` from
/// `Toasty.toml` through here instead.
pub const DUMP_TABLE_NAME_PREFIX_ENV: &str = "TOASTY_DUMP_TABLE_NAME_PREFIX";

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

#[cfg(debug_assertions)]
mod dump {
    use super::*;
    use crate::schema::DiscoverItem;
    use toasty_core::schema::app::ModelSet;

    /// Flavor names accepted in [`DUMP_SCHEMA_ENV`]; the CLI's `Flavor::as_str`
    /// must match.
    const FLAVORS: &[(&str, &Capability)] = &[
        ("sqlite", &Capability::SQLITE),
        ("postgresql", &Capability::POSTGRESQL),
        ("mysql", &Capability::MYSQL),
        ("mariadb", &Capability::MARIADB),
        ("turso", &Capability::TURSO),
    ];

    #[linktime::ctor(unsafe)]
    fn __toasty_maybe_dump_schema() {
        let Some(flavor) = std::env::var_os(DUMP_SCHEMA_ENV) else {
            return;
        };

        // Never fall through to the user's `main`.
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
        let Some(&(_, capability)) = FLAVORS.iter().find(|(name, _)| *name == flavor) else {
            let names: Vec<_> = FLAVORS.iter().map(|(name, _)| *name).collect();
            return Err(format!(
                "`{DUMP_SCHEMA_ENV}={flavor}` names an unknown flavor; expected one of: {}",
                names.join(", ")
            ));
        };

        // Sort by name so the schema is stable across rebuilds; `inventory`
        // iteration order depends on link order.
        let mut model_set = ModelSet::new();
        for item in crate::schema::inventory::iter::<DiscoverItem> {
            item.add_to(&mut model_set);
        }

        let mut models: Vec<_> = model_set.into_iter().collect();
        models.sort_by(|a, b| a.name().parts.cmp(&b.name().parts));

        let app_schema = toasty_core::schema::app::Schema::from_macro(models)
            .map_err(|err| format!("failed to build schema from registered models: {err}"))?;

        let mut builder = toasty_core::schema::Builder::new();

        if let Some(prefix) = std::env::var_os(DUMP_TABLE_NAME_PREFIX_ENV) {
            builder.table_name_prefix(&prefix.to_string_lossy());
        }

        let schema = builder
            .build(app_schema, capability)
            .map_err(|err| format!("failed to lower schema for flavor `{flavor}`: {err}"))?;

        serde_json::to_string(&SchemaDump {
            version: SCHEMA_DUMP_VERSION,
            schema: schema.db,
        })
        .map_err(|err| format!("failed to serialize schema: {err}"))
    }
}
