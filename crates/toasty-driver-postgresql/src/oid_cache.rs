use std::collections::HashMap;

use toasty_core::{Result, schema::db};
use tokio_postgres::{
    Client,
    types::{Kind, Type},
};

use crate::r#type::TypeExt;

/// Lazily caches PostgreSQL OIDs for native enum types.
///
/// On first encounter of an enum parameter type, queries `pg_type` for the
/// OID and constructs a `tokio_postgres::types::Type` with `Kind::Enum`.
/// Subsequent lookups for the same type name return the cached value.
#[derive(Debug, Default)]
pub struct OidCache {
    enum_types: HashMap<String, Type>,
}

impl OidCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve a `db::Type` to a PostgreSQL wire type. For native enum types,
    /// lazily queries `pg_type` for the OID and caches the result.
    pub async fn resolve(&mut self, client: &Client, ty: &db::Type) -> Result<Type> {
        if let db::Type::Enum(type_enum) = ty
            && let Some(name) = &type_enum.name
        {
            // Check cache first
            if let Some(pg_type) = self.enum_types.get(name) {
                return Ok(pg_type.clone());
            }

            // Query pg_type for the OID
            let oid_row = client
                .query_one("SELECT oid FROM pg_type WHERE typname = $1", &[name])
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?;
            let oid: u32 = oid_row.get(0);
            let variants: Vec<String> = type_enum.variants.iter().map(|v| v.name.clone()).collect();
            let pg_type = Type::new(
                name.clone(),
                oid,
                Kind::Enum(variants),
                "public".to_string(),
            );
            self.enum_types.insert(name.clone(), pg_type.clone());
            return Ok(pg_type);
        }

        Ok(ty.to_postgres_type())
    }
}
