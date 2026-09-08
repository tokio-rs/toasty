use hashbrown::{HashMap, HashSet};

use toasty_core::{Result, schema::db};
use tokio_postgres::{
    Client,
    types::{Kind, Type},
};

use crate::r#type::{array_type_of, to_postgres_type};

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
        let mut names = HashSet::new();
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
        // array type, and `pg_enum` gives us the variant labels. Fetching
        // all three in one query keeps preload to a single round-trip and
        // makes the cached `Type` indistinguishable from one tokio_postgres
        // would build itself — so future `Type` equality / `Kind` checks
        // can't trip over an empty variant list.
        //
        // `to_regtype` resolves each name through the session's
        // `search_path`, exactly as an unqualified name in a statement
        // would. Matching `pg_type.typname` directly would also match
        // same-named enums in schemas outside the search path — under
        // schema-per-tenant setups the cache could then bind the wrong
        // schema's OID.
        let rows = client
            .query(
                "SELECT t.typname, t.oid, t.typarray, n.nspname, \
                        array_agg(e.enumlabel ORDER BY e.enumsortorder) \
                 FROM unnest($1::text[]) AS name \
                 JOIN pg_type t ON t.oid = to_regtype(quote_ident(name))::oid \
                 JOIN pg_namespace n ON n.oid = t.typnamespace \
                 JOIN pg_enum e ON e.enumtypid = t.oid \
                 GROUP BY t.typname, t.oid, t.typarray, n.nspname",
                &[&uncached],
            )
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        for row in &rows {
            let name: String = row.get(0);
            let oid: u32 = row.get(1);
            let array_oid: u32 = row.get(2);
            let schema: String = row.get(3);
            let variants: Vec<String> = row.get(4);

            let enum_type = Type::new(name.clone(), oid, Kind::Enum(variants), schema.clone());
            let array_type = Type::new(
                format!("_{name}"),
                array_oid,
                Kind::Array(enum_type.clone()),
                schema,
            );
            self.enum_types.insert(name.clone(), enum_type);
            self.enum_array_types.insert(name, array_type);
        }

        Ok(())
    }

    /// Look up the PostgreSQL wire type for a `db::Type`. Recurses into
    /// `List(elem)` so list-of-enum and list-of-scalar both resolve to the
    /// correct array OID. Panics if an enum type was not preloaded.
    pub fn get(&self, ty: &db::Type) -> &Type {
        match ty {
            db::Type::Enum(type_enum) if type_enum.name.is_some() => {
                let name = type_enum.name.as_ref().unwrap();
                self.enum_types.get(name).unwrap_or_else(|| {
                    panic!("enum type '{name}' not preloaded — call preload() before get()")
                })
            }
            db::Type::List(elem) => match elem.as_ref() {
                db::Type::Enum(type_enum) if type_enum.name.is_some() => {
                    let name = type_enum.name.as_ref().unwrap();
                    self.enum_array_types.get(name).unwrap_or_else(|| {
                        panic!(
                            "enum array type '_{name}' not preloaded — call preload() before get()"
                        )
                    })
                }
                _ => array_type_of(self.get(elem)),
            },
            _ => to_postgres_type(ty),
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

#[cfg(test)]
mod tests {
    use super::OidCache;
    use std::sync::atomic::{AtomicU64, Ordering};
    use toasty_core::schema::db;
    use tokio_postgres::{
        Client, NoTls,
        types::{Kind, Type},
    };

    /// Connect to the PG instance pointed at by `TOASTY_TEST_POSTGRES_URL`.
    /// Returns `None` when the env var is unset so `cargo test` stays green
    /// without a running database; CI sets the variable so the tests run.
    async fn try_connect() -> Option<Client> {
        let url = std::env::var("TOASTY_TEST_POSTGRES_URL").ok()?;
        let (client, conn) = tokio_postgres::connect(&url, NoTls)
            .await
            .expect("PG connection failed");
        tokio::spawn(async move {
            let _ = conn.await;
        });
        Some(client)
    }

    /// Per-process, per-test enum name so concurrent tests don't collide on
    /// the shared PG instance.
    fn enum_name(tag: &str) -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("toasty_oid_{tag}_{}_{n}", std::process::id())
    }

    async fn create_enum(client: &Client, name: &str, variants: &[&str]) {
        client
            .simple_query(&format!("DROP TYPE IF EXISTS {name}"))
            .await
            .unwrap();
        let labels = variants
            .iter()
            .map(|v| format!("'{v}'"))
            .collect::<Vec<_>>()
            .join(", ");
        client
            .simple_query(&format!("CREATE TYPE {name} AS ENUM ({labels})"))
            .await
            .unwrap();
    }

    async fn drop_enum(client: &Client, name: &str) {
        let _ = client
            .simple_query(&format!("DROP TYPE IF EXISTS {name}"))
            .await;
    }

    fn db_enum(name: &str) -> db::Type {
        db::Type::Enum(db::TypeEnum {
            name: Some(name.to_string()),
            variants: vec![],
        })
    }

    /// preload populates the cached `Type` with the actual variant labels in
    /// declaration order. Guards against the empty-Vec hack the cache used
    /// to install, which would silently break any future `Type` equality or
    /// kind-introspection check.
    #[tokio::test]
    async fn preload_caches_variants_in_declaration_order() {
        let Some(client) = try_connect().await else {
            return;
        };
        let name = enum_name("variants");
        create_enum(&client, &name, &["pending", "active", "done"]).await;

        let mut cache = OidCache::new();
        cache.preload(&client, [&db_enum(&name)]).await.unwrap();

        assert_eq!(
            cache.get(&db_enum(&name)).kind(),
            &Kind::Enum(vec!["pending".into(), "active".into(), "done".into()])
        );

        drop_enum(&client, &name).await;
    }

    /// The cached array type's element is the same `Type` as the cached
    /// scalar — so list-of-enum binds use a `Type` that's
    /// indistinguishable from one tokio_postgres would build itself.
    #[tokio::test]
    async fn preload_caches_array_with_matching_element() {
        let Some(client) = try_connect().await else {
            return;
        };
        let name = enum_name("arr");
        create_enum(&client, &name, &["a", "b"]).await;

        let mut cache = OidCache::new();
        cache.preload(&client, [&db_enum(&name)]).await.unwrap();

        let scalar = cache.get(&db_enum(&name));
        let list = cache.get(&db::Type::List(Box::new(db_enum(&name))));

        assert_eq!(list.name(), format!("_{name}"));
        match list.kind() {
            Kind::Array(elem) => assert_eq!(elem, scalar),
            other => panic!("expected Kind::Array, got {other:?}"),
        }

        drop_enum(&client, &name).await;
    }

    /// preload recurses into `List(Enum)` so a list-of-enum bind param can
    /// be resolved after a single preload of the list type.
    #[tokio::test]
    async fn preload_recurses_into_list_of_enum() {
        let Some(client) = try_connect().await else {
            return;
        };
        let name = enum_name("recurse");
        create_enum(&client, &name, &["x", "y"]).await;

        let mut cache = OidCache::new();
        let list_ty = db::Type::List(Box::new(db_enum(&name)));
        cache.preload(&client, [&list_ty]).await.unwrap();

        // Both lookups succeed without further preloads.
        assert_eq!(
            cache.get(&db_enum(&name)).kind(),
            &Kind::Enum(vec!["x".into(), "y".into()])
        );
        assert_eq!(cache.get(&list_ty).name(), format!("_{name}"));

        drop_enum(&client, &name).await;
    }

    /// `List(scalar)` resolves via `array_type_of` with no preload required.
    #[tokio::test]
    async fn list_of_scalar_resolves_without_preload() {
        let cache = OidCache::new();
        assert_eq!(
            cache.get(&db::Type::List(Box::new(db::Type::Integer(8)))),
            &Type::INT8_ARRAY
        );
        assert_eq!(
            cache.get(&db::Type::List(Box::new(db::Type::Text))),
            &Type::TEXT_ARRAY
        );
    }

    /// Preloading the same enum twice neither errors nor mutates the cache.
    #[tokio::test]
    async fn preload_is_idempotent() {
        let Some(client) = try_connect().await else {
            return;
        };
        let name = enum_name("idem");
        create_enum(&client, &name, &["only"]).await;

        let mut cache = OidCache::new();
        cache.preload(&client, [&db_enum(&name)]).await.unwrap();
        let first = cache.get(&db_enum(&name)).clone();
        cache.preload(&client, [&db_enum(&name)]).await.unwrap();
        let second = cache.get(&db_enum(&name));
        assert_eq!(&first, second);

        drop_enum(&client, &name).await;
    }

    /// preload resolves enum names through the session `search_path`, so
    /// a same-named enum in another schema cannot shadow the visible one.
    #[tokio::test]
    async fn preload_resolves_via_search_path() {
        let Some(client) = try_connect().await else {
            return;
        };
        let schema_a = enum_name("nsp_a");
        let schema_b = enum_name("nsp_b");
        for (schema, labels) in [(&schema_a, "'a1', 'a2'"), (&schema_b, "'b1'")] {
            client
                .simple_query(&format!("CREATE SCHEMA \"{schema}\""))
                .await
                .unwrap();
            client
                .simple_query(&format!(
                    "CREATE TYPE \"{schema}\".status AS ENUM ({labels})"
                ))
                .await
                .unwrap();
        }

        client
            .simple_query(&format!("SET search_path TO \"{schema_a}\""))
            .await
            .unwrap();
        let mut cache = OidCache::new();
        cache.preload(&client, [&db_enum("status")]).await.unwrap();
        let ty = cache.get(&db_enum("status"));
        assert_eq!(ty.kind(), &Kind::Enum(vec!["a1".into(), "a2".into()]));
        assert_eq!(ty.schema(), schema_a);

        client
            .simple_query(&format!("SET search_path TO \"{schema_b}\""))
            .await
            .unwrap();
        let mut cache = OidCache::new();
        cache.preload(&client, [&db_enum("status")]).await.unwrap();
        assert_eq!(
            cache.get(&db_enum("status")).kind(),
            &Kind::Enum(vec!["b1".into()])
        );

        for schema in [&schema_a, &schema_b] {
            let _ = client
                .simple_query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
                .await;
        }
    }

    /// Multiple uncached enums are resolved in a single round-trip and each
    /// gets its own variant list.
    #[tokio::test]
    async fn preload_resolves_multiple_enums_in_one_call() {
        let Some(client) = try_connect().await else {
            return;
        };
        let a = enum_name("multi_a");
        let b = enum_name("multi_b");
        create_enum(&client, &a, &["one"]).await;
        create_enum(&client, &b, &["red", "blue"]).await;

        let mut cache = OidCache::new();
        let types = [db_enum(&a), db_enum(&b)];
        cache.preload(&client, types.iter()).await.unwrap();

        assert_eq!(
            cache.get(&db_enum(&a)).kind(),
            &Kind::Enum(vec!["one".into()])
        );
        assert_eq!(
            cache.get(&db_enum(&b)).kind(),
            &Kind::Enum(vec!["red".into(), "blue".into()])
        );

        drop_enum(&client, &a).await;
        drop_enum(&client, &b).await;
    }
}
