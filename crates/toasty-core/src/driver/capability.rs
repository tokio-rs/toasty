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

    /// SQLite capabilities.
    pub const SQLITE: Self = Self {
        sql: true,
        storage_types: StorageTypes::SQLITE,
        cte_with_update: false,
        select_for_update: false,
        primary_key_ne_predicate: true,
    };

    /// PostgreSQL capabilities
    pub const POSTGRESQL: Self = Self {
        cte_with_update: true,
        storage_types: StorageTypes::POSTGRESQL,
        select_for_update: true,
        ..Self::SQLITE
    };

    /// MySQL capabilities
    pub const MYSQL: Self = Self {
        cte_with_update: false,
        storage_types: StorageTypes::MYSQL,
        select_for_update: true,
        ..Self::SQLITE
    };

    /// DynamoDB capabilities
    pub const DYNAMODB: Self = Self {
        sql: false,
        storage_types: StorageTypes::DYNAMODB,
        cte_with_update: false,
        select_for_update: false,
        primary_key_ne_predicate: false,
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

    pub const POSTGRESQL: StorageTypes = StorageTypes {
        default_string_type: db::Type::Text,

        // The maximum n you can specify is 10 485 760 characters. Attempts to
        // declare varchar with a larger typmod will be rejected at
        // table‐creation time.
        varchar: Some(10_485_760),

        default_uuid_type: db::Type::Uuid,

        // PostgreSQL has native support for temporal types
        default_timestamp_type: db::Type::Timestamp,
        default_zoned_type: db::Type::Timestamp,
        default_date_type: db::Type::Date,
        default_time_type: db::Type::Time,
        default_datetime_type: db::Type::DateTime,

        // PostgreSQL BIGINT is signed 64-bit, so unsigned integers are limited
        // to i64::MAX. While NUMERIC could theoretically support larger values,
        // we prefer explicit limits over implicit type switching.
        max_unsigned_integer: Some(i64::MAX as u64),
    };

    pub const MYSQL: StorageTypes = StorageTypes {
        default_string_type: db::Type::VarChar(191),

        // Values in VARCHAR columns are variable-length strings. The length can
        // be specified as a value from 0 to 65,535. The effective maximum
        // length of a VARCHAR is subject to the maximum row size (65,535 bytes,
        // which is shared among all columns) and the character set used.
        varchar: Some(65_535),

        // MySQL does not have an inbuilt UUID type. The binary blob type is more
        // difficult to read than Text but likely has better performance characteristics.
        default_uuid_type: db::Type::Binary(16),

        // MySQL has native support for temporal types
        default_timestamp_type: db::Type::Timestamp,
        default_zoned_type: db::Type::Timestamp,
        default_date_type: db::Type::Date,
        default_time_type: db::Type::Time,
        default_datetime_type: db::Type::DateTime,

        // MySQL supports full u64 range via BIGINT UNSIGNED
        max_unsigned_integer: None,
    };

    pub const DYNAMODB: StorageTypes = StorageTypes {
        default_string_type: db::Type::Text,

        // DynamoDB does not support varchar types
        varchar: None,

        default_uuid_type: db::Type::Blob,

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
