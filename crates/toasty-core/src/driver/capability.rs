use crate::schema::db;

#[derive(Debug)]
pub struct Capability {
    /// When true, the database uses a SQL-based query language.
    pub sql: bool,

    /// Column storage types supported by the database
    pub storage_types: StorageTypes,

    /// SQL: supports update statements in CTE queries.
    pub cte_with_update: bool,

    /// SQL: Supports row-level locking. If false, then the driver is expected
    /// to serializable transaction-level isolation.
    pub select_for_update: bool,

    /// DynamoDB does not support != predicates on the primary key.
    pub primary_key_ne_predicate: bool,

    /// Whether BigDecimal driver support is implemented.
    /// TODO: Remove this flag when PostgreSQL BigDecimal support is implemented.
    /// Currently only MySQL has implemented BigDecimal driver support.
    pub bigdecimal_implemented: bool,
}

#[derive(Debug)]
pub struct StorageTypes {
    /// The default storage type for a string.
    pub default_string_type: db::Type,

    /// When `Some` the database supports varchar types with the specified upper
    /// limit.
    pub varchar: Option<u64>,

    /// The default storage type for a UUID.
    pub default_uuid_type: db::Type,

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

    /// Whether the database has native support for Timestamp types.
    pub native_timestamp: bool,

    /// Whether the database has native support for Date types.
    pub native_date: bool,

    /// Whether the database has native support for Time types.
    pub native_time: bool,

    /// Whether the database has native support for DateTime types.
    pub native_datetime: bool,

    /// Whether the database has native support for Decimal types.
    pub native_decimal: bool,

    /// Whether the database's decimal type supports arbitrary precision.
    /// When false, the decimal type requires fixed precision and scale to be specified upfront.
    /// - PostgreSQL: true (NUMERIC supports arbitrary precision)
    /// - MySQL: false (DECIMAL requires fixed precision/scale)
    /// - SQLite/DynamoDB: false (no native decimal support, stored as TEXT)
    pub decimal_arbitrary_precision: bool,

    /// Maximum value for unsigned integers. When `Some`, unsigned integers
    /// are limited to this value. When `None`, full u64 range is supported.
    pub max_unsigned_integer: Option<u64>,
}

impl Capability {
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

    /// A set of capabilities used for predictable unit tests, e.g. those testing schema building,
    /// that do not require an actual running database instance.
    pub const TEST_CAPABILITY: Self = Self {
        sql: true,
        cte_with_update: false,
        select_for_update: false,
        primary_key_ne_predicate: true,
        bigdecimal_implemented: false,
        storage_types: StorageTypes {
            default_string_type: db::Type::Text,

            varchar: Some(1_000_000_000),

            default_uuid_type: db::Type::Blob,

            default_decimal_type: db::Type::Text,
            default_bigdecimal_type: db::Type::Text,

            default_timestamp_type: db::Type::Text,
            default_zoned_type: db::Type::Text,
            default_date_type: db::Type::Text,
            default_time_type: db::Type::Text,
            default_datetime_type: db::Type::Text,

            native_timestamp: false,
            native_date: false,
            native_time: false,
            native_datetime: false,
            native_decimal: false,

            decimal_arbitrary_precision: false,

            max_unsigned_integer: Some(i64::MAX as u64),
        },
    };
}
