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
    /// When `true`, the database uses a SQL-based query language and the
    /// planner will emit [`QuerySql`](super::operation::QuerySql) operations.
    pub sql: bool,

    /// Column storage types supported by the database.
    pub storage_types: StorageTypes,

    /// Schema mutation capabilities supported by the datbase.
    pub schema_mutations: SchemaMutations,

    /// SQL: supports update statements in CTE queries.
    pub cte_with_update: bool,

    /// SQL: Supports row-level locking. If false, then the driver is expected
    /// to serializable transaction-level isolation.
    pub select_for_update: bool,

    /// SQL: Mysql doesn't support returning clauses from insert / update queries
    pub returning_from_mutation: bool,

    /// DynamoDB does not support != predicates on the primary key.
    pub primary_key_ne_predicate: bool,

    /// Whether the database has an auto increment modifier for integer columns.
    pub auto_increment: bool,

    /// Whether the database supports `VARCHAR(n)` column types natively.
    ///
    /// Must be consistent with [`StorageTypes::varchar`]: when `true`,
    /// `varchar` must be `Some`; when `false`, `varchar` must be `None`.
    /// Use [`Capability::validate`] to check this invariant.
    pub native_varchar: bool,

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
    /// PostgreSQL's `^@`). When `false`, the lowering rewrites it to a
    /// `LIKE` expression — which requires `native_like` to be `true`.
    pub native_starts_with: bool,

    /// Whether the database has a native `LIKE` expression. When `false`,
    /// `Expr::Like` cannot be sent to the driver; `starts_with` lowering
    /// will not produce one.
    pub native_like: bool,

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

    /// Whether the driver supports backward (previous-page) pagination.
    /// SQL: true. DynamoDB: false.
    pub backward_pagination: bool,

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
    /// - MySQL / SQLite JSON: `false` — no value-removal operator; RMW
    ///   fallback (future work).
    /// - DynamoDB List: `false` — no value-removal on Lists; RMW fallback
    ///   (future work).
    pub vec_remove: bool,

    /// Whether the driver supports atomic in-place removal of the last
    /// element of a `Vec<scalar>` field (`stmt::pop`).
    ///
    /// - PostgreSQL: `true` — array slicing.
    /// - MySQL / SQLite: future work — `JSON_REMOVE` / `json_remove` with a
    ///   computed-length path expression. Currently `false`.
    /// - DynamoDB: `false` — `UpdateExpression` indices must be literal
    ///   integers, so the last index cannot be expressed in one statement.
    pub vec_pop: bool,

    /// Whether the driver supports atomic in-place removal of an element at a
    /// given index from a `Vec<scalar>` field (`stmt::remove_at`).
    ///
    /// - PostgreSQL: `true` — array slicing.
    /// - MySQL / SQLite: future work — `JSON_REMOVE` / `json_remove` with a
    ///   path expression. Currently `false`.
    /// - DynamoDB: future work — `REMOVE path[i]` for a literal index.
    ///   Currently `false`.
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
/// Used by the migration generator to decide how to express schema changes.
/// For example, SQLite cannot alter column types so migrations must recreate
/// the table instead.
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
        sql: true,
        storage_types: StorageTypes::SQLITE,
        schema_mutations: SchemaMutations::SQLITE,
        cte_with_update: false,
        select_for_update: false,
        returning_from_mutation: true,
        primary_key_ne_predicate: true,
        auto_increment: true,
        bigdecimal_implemented: false,

        native_varchar: true,

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

        native_starts_with: false,
        native_like: true,

        // SQL drivers handle unindexed queries via QuerySql (see field doc).
        scan: true,
        scan_supports_sort: true,

        test_connection_pool: false,

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

        // SQLite renders `IsSuperset` / `Intersects` as `json_each`
        // subqueries that accept any rhs expression.
        native_array_set_predicates: true,

        // SQLite JSON1 has no value-removal operator on JSON arrays, and
        // pop / remove_at need a path expression built from
        // `json_array_length`. Future work.
        vec_remove: false,
        vec_pop: false,
        vec_remove_at: false,
    };

    /// PostgreSQL capabilities
    pub const POSTGRESQL: Self = Self {
        cte_with_update: true,
        storage_types: StorageTypes::POSTGRESQL,
        schema_mutations: SchemaMutations::POSTGRESQL,
        select_for_update: true,
        auto_increment: true,
        bigdecimal_implemented: false,

        // PostgreSQL has the `^@` prefix-match operator.
        native_starts_with: true,

        // PostgreSQL has CREATE TYPE ... AS ENUM
        native_enum: true,
        named_enum_types: true,

        // PostgreSQL has native date/time types
        native_timestamp: true,
        native_date: true,
        native_time: true,
        native_datetime: true,

        // PostgreSQL has native NUMERIC type with arbitrary precision
        native_decimal: true,
        decimal_arbitrary_precision: true,

        test_connection_pool: true,

        // PostgreSQL accepts a single array-valued bind param and supports
        // `expr <op> ANY(array)` / `<op> ALL(array)` predicates.
        bind_list_param: true,
        predicate_match_any: true,

        // PostgreSQL: native arrays (`text[]`, `int8[]`, …) are the storage
        // representation for `Vec<scalar>` model fields.
        native_array: true,
        vec_scalar: true,

        // PostgreSQL: all three collection removals are atomic via native
        // array operators / slicing.
        vec_remove: true,
        vec_pop: true,
        vec_remove_at: true,

        ..Self::SQLITE
    };

    /// MySQL capabilities
    pub const MYSQL: Self = Self {
        cte_with_update: false,
        storage_types: StorageTypes::MYSQL,
        schema_mutations: SchemaMutations::MYSQL,
        select_for_update: true,
        returning_from_mutation: false,
        auto_increment: true,
        bigdecimal_implemented: true,

        // MySQL has inline ENUM('a', 'b') column types
        native_enum: true,
        named_enum_types: false,

        // MySQL has native date/time types
        native_timestamp: true,
        native_date: true,
        native_time: true,
        native_datetime: true,

        // MySQL has DECIMAL type but requires fixed precision/scale upfront
        native_decimal: true,
        decimal_arbitrary_precision: false,

        test_connection_pool: true,

        // `Vec<scalar>` model fields land in a `JSON` column. The driver
        // serializes `Value::List` to a JSON string at bind time, so the
        // extract pass keeps the list as one `Value::List` parameter
        // instead of expanding it (the `InList` branch in
        // `extract_params` covers the `IN (...)` case so this flag does
        // not regress the IN-list rendering).
        bind_list_param: true,
        vec_scalar: true,

        ..Self::SQLITE
    };

    /// DynamoDB capabilities
    pub const DYNAMODB: Self = Self {
        sql: false,
        storage_types: StorageTypes::DYNAMODB,
        schema_mutations: SchemaMutations::DYNAMODB,
        cte_with_update: false,
        select_for_update: false,
        returning_from_mutation: false,
        primary_key_ne_predicate: false,
        auto_increment: false,
        bigdecimal_implemented: false,
        native_varchar: false,
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

        // DynamoDB has `begins_with()` but no LIKE.
        native_starts_with: true,
        native_like: false,

        scan: true,
        scan_supports_sort: false,

        test_connection_pool: false,

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

        // DynamoDB emulates `IsSuperset` / `Intersects` by expanding the rhs
        // into one `contains(path, vN)` clause per element. The expansion
        // requires the rhs to be a `Value::List` at filter-construction time
        // — the capability check rejects any other rhs shape.
        native_array_set_predicates: false,

        // DynamoDB Lists have no atomic value-removal; pop cannot be
        // expressed because `UpdateExpression` indices must be literal
        // integers. Native `remove_at(literal i)` is future work.
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
