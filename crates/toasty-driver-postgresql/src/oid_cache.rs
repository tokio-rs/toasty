use std::collections::{HashMap, HashSet};

use toasty_core::{Result, schema::db};
use tokio_postgres::{
    Client,
    types::{Kind, Type},
};

use crate::r#type::TypeExt;

/// Lazily caches PostgreSQL OIDs for native enum types.
///
/// Call [`preload`](Self::preload) with all types a statement will use, then
/// [`get`](Self::get) synchronously for each parameter.
#[derive(Debug, Default)]
pub struct OidCache {
    enum_types: HashMap<String, Type>,
}

impl OidCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ensure all enum types in `types` are cached. Issues at most one query
    /// to `pg_type` for all uncached names.
    pub async fn preload<'a>(
        &mut self,
        client: &Client,
        types: impl IntoIterator<Item = &'a db::Type>,
    ) -> Result<()> {
        // Collect uncached enum type names, deduplicating
        let uncached: Vec<String> = types
            .into_iter()
            .filter_map(|ty| match ty {
                db::Type::Enum(te) => te.name.clone(),
                _ => None,
            })
            .filter(|name| !self.enum_types.contains_key(name))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        if uncached.is_empty() {
            return Ok(());
        }

        let rows = client
            .query(
                "SELECT typname, oid FROM pg_type WHERE typname = ANY($1)",
                &[&uncached],
            )
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        for row in &rows {
            let name: String = row.get(0);
            let oid: u32 = row.get(1);

            // We don't have the variant list from pg_type, but we don't need it
            // for wire-format purposes — Kind::Enum just needs the variant names
            // for the Type identity. Use an empty list; the OID is what matters.
            let pg_type = Type::new(name.clone(), oid, Kind::Enum(vec![]), "public".to_string());
            self.enum_types.insert(name, pg_type);
        }

        Ok(())
    }

    /// Look up the PostgreSQL wire type for a `db::Type`. For enum types,
    /// returns the cached type with the correct OID. Panics if an enum type
    /// was not preloaded.
    pub fn get(&self, ty: &db::Type) -> Type {
        if let db::Type::Enum(type_enum) = ty
            && let Some(name) = &type_enum.name
        {
            return self
                .enum_types
                .get(name)
                .unwrap_or_else(|| {
                    panic!("enum type '{name}' not preloaded — call preload() before get()")
                })
                .clone();
        }

        ty.to_postgres_type()
    }
}
