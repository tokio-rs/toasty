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
}

impl Capability {
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

        // SQLite doesn’t really enforce the “N” in VARCHAR(N) at all – it
        // treats any type containing “CHAR”, “CLOB”, or “TEXT” as having TEXT
        // affinity, and simply ignores the length specifier. In other words,
        // whether you declare a column as VARCHAR(10), VARCHAR(1000000), or
        // just TEXT, SQLite won’t truncate or complain based on that number.
        //
        // Instead, the only hard limit on how big a string (or BLOB) can be is
        // the SQLITE_MAX_LENGTH parameter, which is set to 1 billion by default.
        varchar: Some(1_000_000_000),
    };

    pub const POSTGRESQL: StorageTypes = StorageTypes {
        default_string_type: db::Type::Text,

        // The maximum n you can specify is 10 485 760 characters. Attempts to
        // declare varchar with a larger typmod will be rejected at
        // table‐creation time.
        varchar: Some(10_485_760),
    };

    pub const MYSQL: StorageTypes = StorageTypes {
        default_string_type: db::Type::VarChar(191),

        // Values in VARCHAR columns are variable-length strings. The length can
        // be specified as a value from 0 to 65,535. The effective maximum
        // length of a VARCHAR is subject to the maximum row size (65,535 bytes,
        // which is shared among all columns) and the character set used.
        varchar: Some(65_535),
    };

    pub const DYNAMODB: StorageTypes = StorageTypes {
        default_string_type: db::Type::Text,

        // DynamoDB does not support varchar types
        varchar: None,
    };
}
