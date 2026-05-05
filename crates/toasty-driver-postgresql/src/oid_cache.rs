use hashbrown::{HashMap, HashSet};

use toasty_core::{Result, schema::db};
use tokio_postgres::{
    Client,
    types::{Kind, Type},
};

use crate::r#type::{TypeExt, array_type_of};

/// Lazily caches PostgreSQL OIDs for native enum types — both the base enum
/// (e.g. `status`) and its array form (`_status`). Array OIDs are needed
/// when binding `Value::List` of enum values for `= ANY($1)`.
///
/// Call [`preload`](Self::preload) with all types a statement will use, then
/// [`get`](Self::get) synchronously for each parameter.
#[derive(Debug, Default)]
pub struct OidCache {
    /// Base enum type, indexed by typname.
    enum_types: HashMap<String, Type>,
    /// Array-of-enum type, indexed by the base enum's typname.
    enum_array_types: HashMap<String, Type>,
}

impl OidCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ensure all enum types in `types` are cached. Issues at most one query
    /// to `pg_type` for all uncached names. Recurses into `List(...)` so
    /// list-of-enum bind params resolve correctly.
    pub async fn preload<'a>(
        &mut self,
        client: &Client,
        types: impl IntoIterator<Item = &'a db::Type>,
    ) -> Result<()> {
        let mut names = HashSet::<String>::new();
        for ty in types {
            collect_enum_names(ty, &mut names);
        }
        let uncached: Vec<String> = names
            .into_iter()
            .filter(|name| !self.enum_types.contains_key(name))
            .collect();

        if uncached.is_empty() {
            return Ok(());
        }

        // `typarray` links each base type to its automatically-created
        // array type. Fetching both saves a second round-trip when a
        // statement binds an enum array.
        let rows = client
            .query(
                "SELECT typname, oid, typarray FROM pg_type WHERE typname = ANY($1)",
                &[&uncached],
            )
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        for row in &rows {
            let name: String = row.get(0);
            let oid: u32 = row.get(1);
            let array_oid: u32 = row.get(2);

            // We don't have the variant list from pg_type, but we don't
            // need it for wire-format purposes — Kind::Enum just needs the
            // variant names for the Type identity. Use an empty list; the
            // OID is what matters.
            let enum_type = Type::new(name.clone(), oid, Kind::Enum(vec![]), "public".to_string());
            let array_type = Type::new(
                format!("_{name}"),
                array_oid,
                Kind::Array(enum_type.clone()),
                "public".to_string(),
            );
            self.enum_types.insert(name.clone(), enum_type);
            self.enum_array_types.insert(name, array_type);
        }

        Ok(())
    }

    /// Look up the PostgreSQL wire type for a `db::Type`. Recurses into
    /// `List(elem)` so list-of-enum and list-of-scalar both resolve to the
    /// correct array OID. Panics if an enum type was not preloaded.
    pub fn get(&self, ty: &db::Type) -> Type {
        match ty {
            db::Type::Enum(type_enum) if type_enum.name.is_some() => {
                let name = type_enum.name.as_ref().unwrap();
                self.enum_types
                    .get(name)
                    .unwrap_or_else(|| {
                        panic!("enum type '{name}' not preloaded — call preload() before get()")
                    })
                    .clone()
            }
            db::Type::List(elem) => match elem.as_ref() {
                db::Type::Enum(type_enum) if type_enum.name.is_some() => {
                    let name = type_enum.name.as_ref().unwrap();
                    self.enum_array_types
                        .get(name)
                        .unwrap_or_else(|| {
                            panic!(
                                "enum array type '_{name}' not preloaded — call preload() before get()"
                            )
                        })
                        .clone()
                }
                _ => array_type_of(&self.get(elem)),
            },
            _ => ty.to_postgres_type(),
        }
    }
}

/// Walk a `db::Type` and collect every named enum's typname. Recurses into
/// `List(elem)` so list-of-enum bind params get preloaded.
fn collect_enum_names(ty: &db::Type, out: &mut HashSet<String>) {
    match ty {
        db::Type::Enum(te) => {
            if let Some(name) = &te.name {
                out.insert(name.clone());
            }
        }
        db::Type::List(elem) => collect_enum_names(elem, out),
        _ => {}
    }
}
