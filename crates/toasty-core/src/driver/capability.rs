use crate::{schema::db, stmt};

/// Describes what a database driver supports.
///
/// The query planner reads these flags to decide which [`Operation`](super::Operation)
/// variants to generate. For example, a SQL driver sets `sql: true` and
/// receives `QuerySql` operations, while DynamoDB sets `sql: false` and
/// receives key-value operations like `GetByKey` and `QueryPk`.
///
/// Pre-built configurations are available as associated constants:
/// [`SQLITE`](Self::SQLITE), [`POSTGRESQL`](Self::POSTGRESQL),
/// [`MYSQL`](Self::MYSQL), and [`DYNAMODB`](Self::DYNAMODB).
///
/// # Examples
///
/// ```
/// use toasty_core::driver::Capability;
///
/// let cap = &Capability::SQLITE;
/// assert!(cap.sql);
/// assert!(cap.returning_from_mutation);
/// assert!(!cap.select_for_update);
/// ```
#[derive(Debug)]
pub struct Capability {
    /// Human-readable driver name used in diagnostics.
    pub driver_name: &'static str,

    /// When `true`, the database uses a SQL-based query language and the
    /// planner will emit [`QuerySql`](super::operation::QuerySql) operations.
    pub sql: bool,

    /// Placeholder syntax accepted by the driver's SQL bind layer.
    ///
    /// SQL drivers set this to `Some`. Non-SQL drivers set this to `None`.
    pub sql_placeholder: Option<SqlPlaceholder>,

    /// Column storage types supported by the database.
    pub storage_types: StorageTypes,

    /// What the database is able to change about its own schema. See
    /// [`SchemaMutations`] for the individual fields; the migration
    /// generator branches on them to choose between an in-place
    /// `ALTER COLUMN` and a table rebuild, and between one combined
    /// alter statement and several single-property ones.
    pub schema_mutations: SchemaMutations,

    /// SQL: supports update statements in CTE queries.
    pub cte_with_update: bool,

    /// SQL: Supports row-level locking. If false, then the driver is expected
    /// to serializable transaction-level isolation.
    pub select_for_update: bool,

    /// SQL: Mysql doesn't support returning clauses from insert / update queries
    pub returning_from_mutation: bool,

    /// Whether an upsert may target the table's primary key.
    ///
    /// When `false`, the verifier returns `unsupported_feature` before
    /// dispatching a primary-key upsert to the driver.
    pub upsert_primary_key: bool,

    /// Whether an upsert may target a secondary unique constraint.
    ///
    /// The driver must match the exact lowered target columns rather than
    /// reacting to an arbitrary unique conflict.
    pub upsert_unique: bool,

    /// Whether an upsert can apply arbitrary separate `on_create` and
    /// `on_update` assignments.
    ///
    /// A driver with this capability must select the branch atomically within
    /// the database operation; it cannot read first and choose a second write.
    /// Drivers without this capability may still accept branch patterns that
    /// map to native conditional assignments.
    pub upsert_branch_assignments: bool,

    /// Whether an insert-or-ignore upsert suppresses only the selected target's
    /// conflict.
    ///
    /// Other uniqueness conflicts and validation errors must remain errors.
    pub upsert_targeted_ignore: bool,

    /// DynamoDB does not support != predicates on the primary key.
    pub primary_key_ne_predicate: bool,

    /// Whether the database has an auto increment modifier for integer columns.
    pub auto_increment: bool,

    /// Maximum storage width, in bytes, for auto-increment integer columns.
    ///
    /// Backends that require a particular declared type for auto-increment
    /// columns use this to cap the storage type selected from the Rust field
    /// type. SQLite requires the declared type to be `INTEGER` when using
    /// `AUTOINCREMENT`; Toasty's SQLite serializer emits that spelling for
    /// `Integer(4)`.
    pub max_auto_increment_integer_width: Option<u8>,

    /// Maximum byte length for a database identifier (table name, index name,
    /// column name, etc.).
    ///
    /// When `Some(n)`, auto-generated index names that exceed `n` bytes are
    /// truncated and a short stable hash suffix is appended so names remain
    /// unique and deterministic across builds. User-supplied `#[index(name =
    /// "...")]` names are left untouched.
    ///
    /// - MySQL: `Some(64)` — hard error on longer names
    /// - PostgreSQL: `Some(63)` — silently truncates, risking collisions
    /// - SQLite / DynamoDB: `None` — no enforced limit
    pub max_identifier_length: Option<usize>,

    /// Whether the database supports `VARCHAR(n)` column types natively.
    ///
    /// Must be consistent with [`StorageTypes::varchar`]: when `true`,
    /// `varchar` must be `Some`; when `false`, `varchar` must be `None`.
    /// Use [`Capability::validate`] to check this invariant.
    pub native_varchar: bool,

    /// Whether the database supports native `JSON` columns.
    pub native_json: bool,

    /// Whether the database supports native `JSONB` columns.
    pub native_jsonb: bool,

    /// Whether the database has native support for Timestamp types.
    pub native_timestamp: bool,

    /// Whether the database has native support for Date types.
    pub native_date: bool,

    /// Whether the database has native support for Time types.
    pub native_time: bool,

    /// Whether the database has native support for DateTime types.
    pub native_datetime: bool,

    /// Whether the database supports native enum types.
    ///
    /// - PostgreSQL: `true` — `CREATE TYPE ... AS ENUM`
    /// - MySQL: `true` — inline `ENUM('a', 'b')` column type
    /// - SQLite: `false` — uses `TEXT` + `CHECK` constraint
    /// - DynamoDB: `false` — plain string attribute
    pub native_enum: bool,

    /// Whether enum types are standalone named objects requiring separate DDL.
    ///
    /// When `true`, migrations must emit `CREATE TYPE` / `ALTER TYPE` for enum
    /// types. When `false`, enum definitions are inline in column types.
    ///
    /// - PostgreSQL: `true` — `CREATE TYPE <name> AS ENUM (...)`
    /// - MySQL: `false` — inline `ENUM('a', 'b')` on the column
    /// - SQLite: `false`
    /// - DynamoDB: `false`
    pub named_enum_types: bool,

    /// Whether the database has native support for Decimal types.
    pub native_decimal: bool,

    /// Whether BigDecimal driver support is implemented.
    /// TODO: Remove this flag when PostgreSQL BigDecimal support is implemented.
    /// Currently only MySQL has implemented BigDecimal driver support.
    pub bigdecimal_implemented: bool,

    /// Whether the database's decimal type supports arbitrary precision.
    /// When false, the decimal type requires fixed precision and scale to be specified upfront.
    /// - PostgreSQL: true (NUMERIC supports arbitrary precision)
    /// - MySQL: false (DECIMAL requires fixed precision/scale)
    /// - SQLite/DynamoDB: false (no native decimal support, stored as TEXT)
    pub decimal_arbitrary_precision: bool,

    /// Whether OR is supported in index key conditions (e.g. DynamoDB KeyConditionExpression).
    /// DynamoDB: false. All other backends: true (SQL backends never use index key conditions).
    pub index_or_predicate: bool,

    /// Whether the database has a native prefix-match operator that does not
    /// require LIKE-style escaping. When `true`, `starts_with` is left in the
    /// AST and the driver renders it natively (DynamoDB's `begins_with()`,
    /// PostgreSQL's `^@`, SQLite's `GLOB`, MySQL's `LIKE BINARY`). When
    /// `false`, the lowering rewrites it to a `LIKE` expression — which
    /// requires `native_like` to be `true`.
    pub native_starts_with: bool,

    /// Whether `starts_with` should be rendered as a SQLite `GLOB 'prefix*'`
    /// expression. When `true`, `extract_params` escapes GLOB metacharacters
    /// (`*`, `?`, `[`) in the prefix and appends `*`; the serializer emits
    /// `col GLOB ?`. Implies `native_starts_with`.
    pub glob_starts_with: bool,

    /// Whether `starts_with` should be rendered as MySQL `BINARY col LIKE ?
    /// ESCAPE '!'`. When `true`, `extract_params` escapes LIKE metacharacters
    /// using `!` as the escape char and appends `%`; the serializer emits
    /// `BINARY col LIKE ? ESCAPE '!'`. Implies `native_starts_with`.
    pub binary_like_starts_with: bool,

    /// Whether the database has a native `LIKE` expression. When `false`,
    /// `Expr::Like` cannot be sent to the driver; `starts_with` lowering
    /// will not produce one.
    pub native_like: bool,

    /// Whether the database has a native case-insensitive `LIKE` operator
    /// (`ILIKE`). Only PostgreSQL has one.
    ///
    /// Toasty does not emulate `ILIKE` on backends that lack it: `.ilike()`
    /// is a pass-through to the database's own operator. When `native_ilike`
    /// is `false`, the query-verify pass rejects a case-insensitive
    /// `Expr::Like` with an
    /// [`unsupported_feature`](crate::Error::unsupported_feature) error rather
    /// than silently degrading to plain `LIKE`, whose case behavior differs.
    ///
    /// Implies `native_like`.
    pub native_ilike: bool,

    /// Whether the driver can answer queries that don't match any primary key
    /// or index — i.e. supports unindexed full-table reads.
    ///
    /// SQL drivers set this to `true`: unindexed queries go through
    /// [`QuerySql`](super::operation::QuerySql), so the SQL engine handles
    /// them transparently. DynamoDB also sets this to `true`; the planner
    /// emits [`Operation::Scan`](super::Operation::Scan) for the unindexed
    /// case. A hypothetical pure key-value store with no full-scan capability
    /// would set this to `false`.
    pub scan: bool,

    /// Whether scan operations support ordering results.
    ///
    /// SQL drivers do not use `Operation::Scan`, so this is `true` for them
    /// (ordering is handled inside `QuerySql`). DynamoDB's `Scan` API returns
    /// items in an arbitrary order with no server-side sort, so this is `false`
    /// for DynamoDB. When `false`, the planner rejects queries that combine a
    /// scan path with `ORDER BY`.
    pub scan_supports_sort: bool,

    /// Whether to test connection pool behavior.
    /// TODO: We only need this for the `connection_per_clone.rs` test, come up with a better way.
    pub test_connection_pool: bool,

    /// Whether the driver honors non-`Default`
    /// [`TransactionMode`](super::operation::TransactionMode) variants
    /// (`Immediate`, `Exclusive`). Currently `true` only for SQLite, which
    /// maps them to `BEGIN IMMEDIATE` / `BEGIN EXCLUSIVE`. Drivers that
    /// leave this `false` reject non-`Default` modes with
    /// [`Error::unsupported_feature`](crate::Error::unsupported_feature).
    pub transaction_lock_mode: bool,

    /// Whether the backend can walk a paginated query in reverse from a
    /// cursor.
    ///
    /// Gates the `prev_cursor` field on a `Page` returned to user code.
    /// When `true`, the executor extracts a previous-page cursor from the
    /// first row of every page (see `apply_sql_pagination` in
    /// `toasty/src/engine/exec/exec_statement.rs`). When `false`, the
    /// executor leaves `prev_cursor` as `None`, so
    /// `Page::has_prev()` returns `false` and `Page::prev(&db)` resolves
    /// to `Ok(None)` without issuing a query. `Paginate::before(cursor)`
    /// itself is not rejected — users who already hold a cursor can walk
    /// backwards explicitly — but a driver that returns `false` is
    /// declaring that it has no way to *produce* such a cursor.
    ///
    /// Drivers should set this to `true` when the backend can answer a
    /// query equivalent to "rows ordered by K, descending from K = c,
    /// limited to N" — i.e. the same `ORDER BY` clause reversed plus a
    /// strict inequality on the cursor key. SQL backends meet this
    /// trivially. DynamoDB does not: a `Query` with `ScanIndexForward =
    /// false` returns rows in the opposite direction but cannot be
    /// rooted at an arbitrary client-supplied cursor without an extra
    /// `KeyConditionExpression`, and `Scan` has no order guarantee at
    /// all.
    pub backward_pagination: bool,

    /// Whether the backend supports `BOOL` as a key attribute type.
    ///
    /// DynamoDB only allows `S`, `N`, or `B` for primary-key and GSI key
    /// attribute types; `BOOL` is rejected at the API level. SQL backends
    /// have no such restriction. When `false`, the schema builder overrides
    /// `storage_ty` for any `Bool` key/index field to `db::Type::Integer(1)`,
    /// letting the engine cast `Bool ↔ I8` and the driver handle it as a
    /// plain number — no driver-level bool-to-number special-casing needed.
    pub bool_key_type: bool,

    /// The driver's bind layer accepts a single parameter whose value is
    /// `Value::List(items)` and type is `Type::List(elem)`, sending it as
    /// one protocol-level parameter (not N separate scalars).
    /// Property of the driver bind impl, not the SQL dialect.
    pub bind_list_param: bool,

    /// The SQL dialect parses `expr <op> ANY(<array>)` and `expr <op> ALL(<array>)`
    /// as predicates against an array-valued operand.
    /// Property of the dialect, not the bind layer.
    pub predicate_match_any: bool,

    /// Whether the database can store a `Vec<scalar>` model field as a native
    /// array column (e.g. PostgreSQL `text[]`, `int8[]`).
    ///
    /// When `true`, schema build maps `Type::List(elem)` to `db::Type::List(elem)`
    /// and the driver's bind layer accepts `Value::List(items)` as a single
    /// array-valued parameter.
    ///
    /// When `false`, `Vec<T>` model fields use whatever fallback the backend
    /// provides (JSON column on MySQL/SQLite, native List `L` on DynamoDB).
    /// See [`Self::vec_scalar`] for the schema-build gate.
    pub native_array: bool,

    /// Whether the driver supports `Vec<scalar>` model fields, by whatever
    /// representation (native typed array column, JSON column, key-value
    /// list attribute, ...). Used by the schema builder as the gate for
    /// accepting `stmt::Type::List(_)` fields.
    pub vec_scalar: bool,

    /// Whether the driver can store a `#[document]` collection field — a
    /// `Vec<T>` of an embedded struct — as a single document column
    /// (`jsonb` / `JSON` on the SQL backends). Used by the schema builder as
    /// the gate for accepting `stmt::Type::List(Document(_))` fields.
    pub document_collections: bool,

    /// Whether the driver natively renders `IsSuperset` / `Intersects` array
    /// predicates over an arbitrary right-hand-side expression.
    ///
    /// SQL drivers set this to `true`: each dialect has a single operator
    /// (`@>` on PostgreSQL, `JSON_CONTAINS` on MySQL, a `json_each`
    /// subquery on SQLite) that takes the rhs as a bound expression
    /// regardless of its shape.
    ///
    /// DynamoDB sets this to `false`: it has no equivalent operator and
    /// emulates the predicates by emitting one `contains(path, vN)` clause
    /// per rhs element, which requires the rhs to be a concrete list of
    /// values at filter-construction time. The capability check rejects
    /// any other rhs shape before the driver is invoked.
    pub native_array_set_predicates: bool,

    /// Whether the driver supports atomic in-place removal of every element
    /// equal to a given value from a `Vec<scalar>` field (`stmt::remove`).
    ///
    /// - PostgreSQL `text[]`: `true` — `array_remove(col, v)`.
    /// - MySQL / SQLite JSON: `false` — no value-removal operator.
    /// - DynamoDB List: `false` — no value-removal on Lists.
    pub vec_remove: bool,

    /// Whether the driver supports atomic in-place removal of the last
    /// element of a `Vec<scalar>` field (`stmt::pop`).
    ///
    /// - PostgreSQL: `true` — array slicing.
    /// - MySQL / SQLite: `false`.
    /// - DynamoDB: `false` — `UpdateExpression` indices must be literal
    ///   integers, so the last index cannot be expressed in one statement.
    pub vec_pop: bool,

    /// Whether the driver supports atomic in-place removal of an element at a
    /// given index from a `Vec<scalar>` field (`stmt::remove_at`).
    ///
    /// - PostgreSQL: `true` — array slicing.
    /// - MySQL / SQLite: `false`.
    /// - DynamoDB: `false`.
    pub vec_remove_at: bool,
}

/// Maps application-level types to the concrete database column types used for
/// storage.
///
/// Each database has different native type support. For example, PostgreSQL has
/// a native `UUID` type while SQLite stores UUIDs as `BLOB`. This struct
/// captures those mappings so the schema layer can generate correct DDL and the
/// driver can encode/decode values appropriately.
///
/// Pre-built configurations: [`SQLITE`](Self::SQLITE),
/// [`POSTGRESQL`](Self::POSTGRESQL), [`MYSQL`](Self::MYSQL),
/// [`DYNAMODB`](Self::DYNAMODB).
///
/// # Examples
///
/// ```
/// use toasty_core::driver::StorageTypes;
///
/// let st = &StorageTypes::POSTGRESQL;
/// // PostgreSQL stores UUIDs natively
/// assert!(matches!(st.default_uuid_type, toasty_core::schema::db::Type::Uuid));
/// ```
#[derive(Debug)]
pub struct StorageTypes {
    /// The default storage type for a string.
    pub default_string_type: db::Type,

    /// When `Some` the database supports varchar types with the specified upper
    /// limit.
    pub varchar: Option<u64>,

    /// The default storage type for a UUID.
    pub default_uuid_type: db::Type,

    /// The default storage type for Bytes (Vec<u8>).
    pub default_bytes_type: db::Type,

    /// The default storage type for a Decimal (fixed-precision decimal).
    pub default_decimal_type: db::Type,

    /// The default storage type for a BigDecimal (arbitrary-precision decimal).
    pub default_bigdecimal_type: db::Type,

    /// The default storage type for a Timestamp (instant in time).
    pub default_timestamp_type: db::Type,

    /// The default storage type for a Zoned (timezone-aware instant).
    pub default_zoned_type: db::Type,

    /// The default storage type for a Date (civil date).
    pub default_date_type: db::Type,

    /// The default storage type for a Time (wall clock time).
    pub default_time_type: db::Type,

    /// The default storage type for a DateTime (civil datetime).
    pub default_datetime_type: db::Type,

    /// Maximum value for unsigned integers. When `Some`, unsigned integers
    /// are limited to this value. When `None`, full u64 range is supported.
    pub max_unsigned_integer: Option<u64>,
}

/// The database's capabilities to mutate the schema (tables, columns, indices).
///
/// Used by the migration generator to decide how to express each
/// column change. `alter_column_type` gates whether an in-place
/// `ALTER COLUMN` is possible at all — SQLite has it set to `false`,
/// and a type change there triggers a full table rebuild (create
/// new table, copy rows, drop old). `alter_column_properties_atomic`
/// decides whether several column-property changes (rename, retype,
/// `NOT NULL`, default) collapse into one statement or emit one per
/// property. MySQL sets both to `true`; PostgreSQL alters in place
/// but requires one statement per property.
///
/// Pre-built configurations: [`SQLITE`](Self::SQLITE),
/// [`POSTGRESQL`](Self::POSTGRESQL), [`MYSQL`](Self::MYSQL),
/// [`DYNAMODB`](Self::DYNAMODB).
///
/// # Examples
///
/// Access through [`Capability::schema_mutations`]:
///
/// ```
/// use toasty_core::driver::Capability;
///
/// let cap = &Capability::POSTGRESQL;
/// assert!(cap.schema_mutations.alter_column_type);
/// assert!(!cap.schema_mutations.alter_column_properties_atomic);
/// ```
#[derive(Debug)]
pub struct SchemaMutations {
    /// Whether the database can change the type of an existing column.
    pub alter_column_type: bool,

    /// Whether the database can change name, type and constraints of a column all
    /// withing a single statement.
    pub alter_column_properties_atomic: bool,
}

/// SQL bind-parameter placeholder syntax accepted by a driver.
///
/// This describes the SQL text users must write when sending raw SQL through
/// [`RawSql`](super::operation::RawSql). The SQL serializer uses the same
/// value when rendering Toasty-generated SQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlPlaceholder {
    /// Positional `?` placeholders, where parameter order is the occurrence
    /// order in the SQL string.
    QuestionMark,

    /// Numbered `?1`, `?2`, ... placeholders.
    NumberedQuestionMark,

    /// Numbered `$1`, `$2`, ... placeholders.
    DollarNumber,
}

impl Capability {
    /// Validates the consistency of the capability configuration.
    ///
    /// This performs sanity checks to ensure the capability fields are
    /// internally consistent. For example, if `native_varchar` is true,
    /// then `storage_types.varchar` must be Some, and vice versa.
    ///
    /// Returns an error if any inconsistencies are found.
    pub fn validate(&self) -> crate::Result<()> {
        // Validate varchar consistency
        if self.native_varchar && self.storage_types.varchar.is_none() {
            return Err(crate::Error::invalid_driver_configuration(
                "native_varchar is true but storage_types.varchar is None",
            ));
        }

        if !self.native_varchar && self.storage_types.varchar.is_some() {
            return Err(crate::Error::invalid_driver_configuration(
                "native_varchar is false but storage_types.varchar is Some",
            ));
        }

        // ILIKE is a case-insensitive LIKE; a backend cannot offer it without
        // a native LIKE.
        if self.native_ilike && !self.native_like {
            return Err(crate::Error::invalid_driver_configuration(
                "native_ilike is true but native_like is false",
            ));
        }

        if self.glob_starts_with && !self.native_starts_with {
            return Err(crate::Error::invalid_driver_configuration(
                "glob_starts_with is true but native_starts_with is false",
            ));
        }

        if self.binary_like_starts_with && !self.native_starts_with {
            return Err(crate::Error::invalid_driver_configuration(
                "binary_like_starts_with is true but native_starts_with is false",
            ));
        }

        if self.glob_starts_with && self.binary_like_starts_with {
            return Err(crate::Error::invalid_driver_configuration(
                "glob_starts_with and binary_like_starts_with cannot both be true",
            ));
        }

        if self.sql && self.sql_placeholder.is_none() {
            return Err(crate::Error::invalid_driver_configuration(
                "sql is true but sql_placeholder is None",
            ));
        }

        if !self.sql && self.sql_placeholder.is_some() {
            return Err(crate::Error::invalid_driver_configuration(
                "sql is false but sql_placeholder is Some",
            ));
        }

        Ok(())
    }

    /// Returns the default string length limit for this database.
    ///
    /// This is useful for tests and applications that need to respect
    /// database-specific string length constraints.
    pub fn default_string_max_length(&self) -> Option<u64> {
        match &self.storage_types.default_string_type {
            db::Type::VarChar(len) => Some(*len),
            _ => None, // Handle other types gracefully
        }
    }

    /// Returns the native database type for an application-level type.
    ///
    /// If the database supports the type natively, returns the same type.
    /// Otherwise, returns the bridge/storage type that the application type
    /// maps to in this database.
    ///
    /// This uses the existing `db::Type::bridge_type()` method to determine
    /// the appropriate bridge type based on the database's storage capabilities.
    pub fn native_type_for(&self, ty: &stmt::Type) -> stmt::Type {
        match ty {
            stmt::Type::Uuid => self.storage_types.default_uuid_type.bridge_type(ty),
            #[cfg(feature = "jiff")]
            stmt::Type::Timestamp => self.storage_types.default_timestamp_type.bridge_type(ty),
            #[cfg(feature = "jiff")]
            stmt::Type::Zoned => self.storage_types.default_zoned_type.bridge_type(ty),
            #[cfg(feature = "jiff")]
            stmt::Type::Date => self.storage_types.default_date_type.bridge_type(ty),
            #[cfg(feature = "jiff")]
            stmt::Type::Time => self.storage_types.default_time_type.bridge_type(ty),
            #[cfg(feature = "jiff")]
            stmt::Type::DateTime => self.storage_types.default_datetime_type.bridge_type(ty),
            _ => ty.clone(),
        }
    }

    /// SQLite capabilities.
    pub const SQLITE: Self = Self {
        driver_name: "SQLite",
        sql: true,
        sql_placeholder: Some(SqlPlaceholder::NumberedQuestionMark),
        storage_types: StorageTypes::SQLITE,
        schema_mutations: SchemaMutations::SQLITE,
        cte_with_update: false,
        select_for_update: false,
        returning_from_mutation: true,
        upsert_primary_key: true,
        upsert_unique: true,
        upsert_branch_assignments: true,
        upsert_targeted_ignore: true,
        primary_key_ne_predicate: true,
        auto_increment: true,
        max_auto_increment_integer_width: Some(4),
        bigdecimal_implemented: false,
        bool_key_type: true,
        max_identifier_length: None,

        native_varchar: true,
        native_json: false,
        native_jsonb: false,

        // SQLite does not have native enum types; uses TEXT + CHECK
        native_enum: false,
        named_enum_types: false,

        // SQLite does not have native date/time types
        native_timestamp: false,
        native_date: false,
        native_time: false,
        native_datetime: false,

        // SQLite does not have native decimal types
        native_decimal: false,
        decimal_arbitrary_precision: false,

        index_or_predicate: true,

        // SQLite's GLOB operator is case-sensitive and is used for starts_with.
        // LIKE is preserved for user-supplied `.like()` calls.
        native_starts_with: true,
        glob_starts_with: true,
        binary_like_starts_with: false,
        native_like: true,

        // SQLite's `LIKE` is case-insensitive for ASCII only; it has no
        // `ILIKE` operator, so `.ilike()` is rejected here.
        native_ilike: false,

        // SQL drivers handle unindexed queries via QuerySql (see field doc).
        scan: true,
        scan_supports_sort: true,

        test_connection_pool: false,

        // SQLite exposes `BEGIN DEFERRED|IMMEDIATE|EXCLUSIVE` for
        // lock-acquisition policy.
        transaction_lock_mode: true,

        backward_pagination: true,

        // `Vec<scalar>` model fields land in a `TEXT` column holding a JSON
        // document (JSON1 extension). The driver serializes `Value::List`
        // to a JSON string at bind time, so the extract pass keeps the list
        // as one `Value::List` parameter; the `InList` branch in
        // `extract_params` covers the `IN (...)` case so this flag does
        // not regress IN-list rendering. The predicate-side `ANY` rewrite
        // is gated on `predicate_match_any`, which stays `false`, so
        // `Path::contains` lowers to a `json_each` subquery instead.
        bind_list_param: true,
        predicate_match_any: false,

        // SQLite has no native typed-array column type; `Vec<scalar>`
        // model fields are stored as a JSON document in a `TEXT` column.
        native_array: false,
        vec_scalar: true,
        document_collections: true,

        // SQLite renders `IsSuperset` / `Intersects` as `json_each`
        // subqueries that accept any rhs expression.
        native_array_set_predicates: true,

        // SQLite JSON1 has no value-removal operator on JSON arrays; pop
        // and remove_at need a path expression built from
        // `json_array_length`.
        vec_remove: false,
        vec_pop: false,
        vec_remove_at: false,
    };

    /// PostgreSQL capabilities
    pub const POSTGRESQL: Self = Self {
        driver_name: "PostgreSQL",
        cte_with_update: true,
        sql_placeholder: Some(SqlPlaceholder::DollarNumber),
        storage_types: StorageTypes::POSTGRESQL,
        schema_mutations: SchemaMutations::POSTGRESQL,
        select_for_update: true,
        auto_increment: true,
        max_auto_increment_integer_width: None,
        bigdecimal_implemented: false,
        max_identifier_length: Some(63),

        // PostgreSQL has the `^@` prefix-match operator.
        native_starts_with: true,
        glob_starts_with: false,
        binary_like_starts_with: false,

        // PostgreSQL is the only backend with a native `ILIKE` operator.
        native_ilike: true,

        // PostgreSQL has CREATE TYPE ... AS ENUM
        native_enum: true,
        named_enum_types: true,
        native_json: true,
        native_jsonb: true,

        // PostgreSQL has native date/time types
        native_timestamp: true,
        native_date: true,
        native_time: true,
        native_datetime: true,

        // PostgreSQL has native NUMERIC type with arbitrary precision
        native_decimal: true,
        decimal_arbitrary_precision: true,

        test_connection_pool: true,

        // PostgreSQL has no SQLite-style lock-mode keyword on BEGIN.
        transaction_lock_mode: false,

        // PostgreSQL accepts a single array-valued bind param and supports
        // `expr <op> ANY(array)` / `<op> ALL(array)` predicates.
        bind_list_param: true,
        predicate_match_any: true,

        // PostgreSQL: native arrays (`text[]`, `int8[]`, …) are the storage
        // representation for `Vec<scalar>` model fields.
        native_array: true,
        vec_scalar: true,
        document_collections: true,

        // PostgreSQL: all three collection removals are atomic via native
        // array operators / slicing.
        vec_remove: true,
        vec_pop: true,
        vec_remove_at: true,

        ..Self::SQLITE
    };

    /// MySQL capabilities
    pub const MYSQL: Self = Self {
        driver_name: "MySQL",
        cte_with_update: false,
        sql_placeholder: Some(SqlPlaceholder::QuestionMark),
        storage_types: StorageTypes::MYSQL,
        schema_mutations: SchemaMutations::MYSQL,
        select_for_update: true,
        returning_from_mutation: false,
        upsert_primary_key: false,
        upsert_unique: false,
        upsert_branch_assignments: false,
        upsert_targeted_ignore: false,
        auto_increment: true,
        max_auto_increment_integer_width: None,
        bigdecimal_implemented: true,
        max_identifier_length: Some(64),

        // MySQL has inline ENUM('a', 'b') column types
        native_enum: true,
        named_enum_types: false,
        native_json: true,

        // MySQL has native date/time types
        native_timestamp: true,
        native_date: true,
        native_time: true,
        native_datetime: true,

        // MySQL has DECIMAL type but requires fixed precision/scale upfront
        native_decimal: true,
        decimal_arbitrary_precision: false,

        test_connection_pool: true,

        // MySQL has no SQLite-style lock-mode keyword on START TRANSACTION.
        transaction_lock_mode: false,

        // `Vec<scalar>` model fields land in a `JSON` column. The driver
        // serializes `Value::List` to a JSON string at bind time, so the
        // extract pass keeps the list as one `Value::List` parameter
        // instead of expanding it (the `InList` branch in
        // `extract_params` covers the `IN (...)` case so this flag does
        // not regress the IN-list rendering).
        bind_list_param: true,
        vec_scalar: true,
        document_collections: true,

        // MySQL uses BINARY col LIKE ? ESCAPE '!' for case-sensitive starts_with.
        glob_starts_with: false,
        binary_like_starts_with: true,

        ..Self::SQLITE
    };

    /// Turso capabilities.
    ///
    /// Identical to [`SQLITE`](Self::SQLITE) at the flag level. The driver
    /// extends SQLite's behavior in two ways that don't fit a capability
    /// bit:
    ///
    /// * It opens a real async connection per pool slot (sharing a cached
    ///   `Database` across `connect()` calls), so the connection-pool test
    ///   suite applies.
    /// * When `Turso::concurrent_writes()` is enabled, the driver issues
    ///   `BEGIN CONCURRENT` for `TransactionMode::Default`, opting the
    ///   transaction into Turso's MVCC concurrency. The other
    ///   `TransactionMode` variants pass through to the SQLite serializer
    ///   unchanged, so callers can still request the classic locking
    ///   strategies per transaction.
    pub const TURSO: Self = Self {
        driver_name: "Turso",
        test_connection_pool: true,
        ..Self::SQLITE
    };

    /// DynamoDB capabilities
    pub const DYNAMODB: Self = Self {
        driver_name: "DynamoDB",
        sql: false,
        sql_placeholder: None,
        storage_types: StorageTypes::DYNAMODB,
        schema_mutations: SchemaMutations::DYNAMODB,
        cte_with_update: false,
        select_for_update: false,
        returning_from_mutation: false,
        upsert_primary_key: true,
        upsert_unique: false,
        upsert_branch_assignments: false,
        upsert_targeted_ignore: true,
        primary_key_ne_predicate: false,
        auto_increment: false,
        max_auto_increment_integer_width: None,
        bigdecimal_implemented: false,
        max_identifier_length: None,
        // DynamoDB key attributes (primary key and GSI keys) only support
        // S, N, or B — BOOL is not a valid key attribute type.
        bool_key_type: false,
        native_varchar: false,
        native_json: false,
        native_jsonb: false,
        native_enum: false,
        named_enum_types: false,

        // DynamoDB does not have native date/time types
        native_timestamp: false,
        native_date: false,
        native_time: false,
        native_datetime: false,

        // DynamoDB does not have native decimal types
        native_decimal: false,
        decimal_arbitrary_precision: false,

        index_or_predicate: false,

        // DynamoDB has `begins_with()` but no LIKE or ILIKE.
        native_starts_with: true,
        glob_starts_with: false,
        binary_like_starts_with: false,
        native_like: false,
        native_ilike: false,

        scan: true,
        scan_supports_sort: false,

        test_connection_pool: false,

        // DynamoDB rejects `Operation::Transaction` wholesale.
        transaction_lock_mode: false,

        backward_pagination: false,

        // DynamoDB: not SQL-based; the array-bind/`ANY`-predicate features do
        // not apply.
        bind_list_param: false,
        predicate_match_any: false,

        // DynamoDB has no SQL-style typed-array column type; the
        // `db::Type::List(elem)` storage shape doesn't apply. `Vec<scalar>`
        // model fields land directly on a List `L` attribute via the driver's
        // `AttributeValue` encoding.
        native_array: false,
        vec_scalar: true,
        // `#[document]` embeds store as a native Map `M` attribute (a
        // `Vec<embed>` collection as a List `L` of Maps). DynamoDB caps
        // attribute nesting at 32 levels; documents deeper than that are not
        // rejected up front — the write surfaces DynamoDB's own error.
        document_collections: true,

        // DynamoDB emulates `IsSuperset` / `Intersects` by expanding the rhs
        // into one `contains(path, vN)` clause per element. The expansion
        // requires the rhs to be a `Value::List` at filter-construction time
        // — the capability check rejects any other rhs shape.
        native_array_set_predicates: false,

        // DynamoDB Lists have no atomic value-removal, and pop cannot be
        // expressed because `UpdateExpression` indices must be literal
        // integers.
        vec_remove: false,
        vec_pop: false,
        vec_remove_at: false,
    };
}

impl StorageTypes {
    /// SQLite storage types
    pub const SQLITE: StorageTypes = StorageTypes {
        default_string_type: db::Type::Text,

        // SQLite doesn't really enforce the "N" in VARCHAR(N) at all – it
        // treats any type containing "CHAR", "CLOB", or "TEXT" as having TEXT
        // affinity, and simply ignores the length specifier. In other words,
        // whether you declare a column as VARCHAR(10), VARCHAR(1000000), or
        // just TEXT, SQLite won't truncate or complain based on that number.
        //
        // Instead, the only hard limit on how big a string (or BLOB) can be is
        // the SQLITE_MAX_LENGTH parameter, which is set to 1 billion by default.
        varchar: Some(1_000_000_000),

        // SQLite does not have an inbuilt UUID type. The binary blob type is more
        // difficult to read than Text but likely has better performance characteristics.
        default_uuid_type: db::Type::Blob,

        default_bytes_type: db::Type::Blob,

        // SQLite does not have a native decimal type. Store as TEXT.
        default_decimal_type: db::Type::Text,
        default_bigdecimal_type: db::Type::Text,

        // SQLite does not have native date/time types. Store as TEXT in ISO 8601 format.
        default_timestamp_type: db::Type::Text,
        default_zoned_type: db::Type::Text,
        default_date_type: db::Type::Text,
        default_time_type: db::Type::Text,
        default_datetime_type: db::Type::Text,

        // SQLite INTEGER is a signed 64-bit integer, so unsigned integers
        // are limited to i64::MAX to prevent overflow
        max_unsigned_integer: Some(i64::MAX as u64),
    };

    /// PostgreSQL storage types.
    pub const POSTGRESQL: StorageTypes = StorageTypes {
        default_string_type: db::Type::Text,

        // The maximum n you can specify is 10 485 760 characters. Attempts to
        // declare varchar with a larger typmod will be rejected at
        // table‐creation time.
        varchar: Some(10_485_760),

        default_uuid_type: db::Type::Uuid,

        default_bytes_type: db::Type::Blob,

        // PostgreSQL has native NUMERIC type for fixed and arbitrary-precision decimals.
        default_decimal_type: db::Type::Numeric(None),
        // TODO: PostgreSQL has native NUMERIC type for arbitrary-precision decimals,
        // but the encoding is complicated and has to be done separately in the future.
        default_bigdecimal_type: db::Type::Text,

        // PostgreSQL has native support for temporal types with microsecond precision (6 digits)
        default_timestamp_type: db::Type::Timestamp(6),
        default_zoned_type: db::Type::Text,
        default_date_type: db::Type::Date,
        default_time_type: db::Type::Time(6),
        default_datetime_type: db::Type::DateTime(6),

        // PostgreSQL BIGINT is signed 64-bit, so unsigned integers are limited
        // to i64::MAX. While NUMERIC could theoretically support larger values,
        // we prefer explicit limits over implicit type switching.
        max_unsigned_integer: Some(i64::MAX as u64),
    };

    /// MySQL storage types.
    pub const MYSQL: StorageTypes = StorageTypes {
        default_string_type: db::Type::VarChar(191),

        // Values in VARCHAR columns are variable-length strings. The length can
        // be specified as a value from 0 to 65,535. The effective maximum
        // length of a VARCHAR is subject to the maximum row size (65,535 bytes,
        // which is shared among all columns) and the character set used.
        varchar: Some(65_535),

        // MySQL does not have an inbuilt UUID type. The binary blob type is
        // more difficult to read than Text but likely has better performance
        // characteristics. However, limitations in the engine make it easier to
        // use VarChar for now.
        default_uuid_type: db::Type::VarChar(36),

        default_bytes_type: db::Type::Blob,

        // MySQL does not have an arbitrary-precision decimal type. The DECIMAL type
        // requires a fixed precision and scale to be specified upfront. Store as TEXT.
        default_decimal_type: db::Type::Text,
        default_bigdecimal_type: db::Type::Text,

        // MySQL has native support for temporal types with microsecond precision (6 digits)
        // The `TIMESTAMP` time only supports a limited range (1970-2038), so we default to
        // DATETIME and let Toasty do the UTC conversion.
        default_timestamp_type: db::Type::DateTime(6),
        default_zoned_type: db::Type::Text,
        default_date_type: db::Type::Date,
        default_time_type: db::Type::Time(6),
        default_datetime_type: db::Type::DateTime(6),

        // MySQL supports full u64 range via BIGINT UNSIGNED
        max_unsigned_integer: None,
    };

    /// DynamoDB storage types.
    pub const DYNAMODB: StorageTypes = StorageTypes {
        default_string_type: db::Type::Text,

        // DynamoDB does not support varchar types
        varchar: None,

        default_uuid_type: db::Type::Text,

        default_bytes_type: db::Type::Blob,

        // DynamoDB does not have a native decimal type. Store as TEXT.
        default_decimal_type: db::Type::Text,
        default_bigdecimal_type: db::Type::Text,

        // DynamoDB does not have native date/time types. Store as TEXT (strings).
        default_timestamp_type: db::Type::Text,
        default_zoned_type: db::Type::Text,
        default_date_type: db::Type::Text,
        default_time_type: db::Type::Text,
        default_datetime_type: db::Type::Text,

        // DynamoDB supports full u64 range (numbers stored as strings)
        max_unsigned_integer: None,
    };
}

impl SchemaMutations {
    /// SQLite schema mutation capabilities. SQLite cannot alter column types.
    pub const SQLITE: Self = Self {
        alter_column_type: false,
        alter_column_properties_atomic: false,
    };

    /// PostgreSQL schema mutation capabilities. Supports altering column types
    /// but not atomically changing multiple column properties.
    pub const POSTGRESQL: Self = Self {
        alter_column_type: true,
        alter_column_properties_atomic: false,
    };

    /// MySQL schema mutation capabilities. Supports altering column types and
    /// atomically changing multiple column properties in a single statement.
    pub const MYSQL: Self = Self {
        alter_column_type: true,
        alter_column_properties_atomic: true,
    };

    /// DynamoDB schema mutation capabilities. Migrations are not currently supported.
    pub const DYNAMODB: Self = Self {
        alter_column_type: false,
        alter_column_properties_atomic: false,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_sqlite_capability() {
        // SQLite has native_varchar=true and varchar=Some, should pass
        assert!(Capability::SQLITE.validate().is_ok());
    }

    #[test]
    fn test_validate_postgresql_capability() {
        // PostgreSQL has native_varchar=true and varchar=Some, should pass
        assert!(Capability::POSTGRESQL.validate().is_ok());
    }

    #[test]
    fn test_validate_mysql_capability() {
        // MySQL has native_varchar=true and varchar=Some, should pass
        assert!(Capability::MYSQL.validate().is_ok());
    }

    #[test]
    fn test_validate_dynamodb_capability() {
        // DynamoDB has native_varchar=false and varchar=None, should pass
        assert!(Capability::DYNAMODB.validate().is_ok());
    }

    #[test]
    fn test_validate_fails_when_sql_has_no_placeholder() {
        let invalid = Capability {
            sql_placeholder: None,
            ..Capability::SQLITE
        };

        let result = invalid.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("sql is true but sql_placeholder is None")
        );
    }

    #[test]
    fn test_validate_fails_when_non_sql_has_placeholder() {
        let invalid = Capability {
            sql_placeholder: Some(SqlPlaceholder::QuestionMark),
            ..Capability::DYNAMODB
        };

        let result = invalid.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("sql is false but sql_placeholder is Some")
        );
    }

    #[test]
    fn test_validate_fails_when_native_varchar_true_but_no_varchar() {
        let invalid = Capability {
            native_varchar: true,
            storage_types: StorageTypes {
                varchar: None, // Invalid: native_varchar is true but varchar is None
                ..StorageTypes::SQLITE
            },
            ..Capability::SQLITE
        };

        let result = invalid.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("native_varchar is true but storage_types.varchar is None")
        );
    }

    #[test]
    fn test_validate_fails_when_native_varchar_false_but_has_varchar() {
        let invalid = Capability {
            native_varchar: false,
            storage_types: StorageTypes {
                varchar: Some(1000), // Invalid: native_varchar is false but varchar is Some
                ..StorageTypes::DYNAMODB
            },
            ..Capability::DYNAMODB
        };

        let result = invalid.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("native_varchar is false but storage_types.varchar is Some")
        );
    }
}
