use crate::{schema::db, stmt};

#[derive(Debug)]
pub struct Capability {
    /// When true, the database uses a SQL-based query language.
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

    pub native_varchar: bool,

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

    /// Maximum value for unsigned integers. When `Some`, unsigned integers
    /// are limited to this value. When `None`, full u64 range is supported.
    pub max_unsigned_integer: Option<u64>,
}

/// The database's capabilities to mutate the schema (tables, columns, indices).
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

        // SQLite does not have native date/time types
        native_timestamp: false,
        native_date: false,
        native_time: false,
        native_datetime: false,

        // SQLite does not have native decimal types
        native_decimal: false,
        decimal_arbitrary_precision: false,
    };

    /// PostgreSQL capabilities
    pub const POSTGRESQL: Self = Self {
        cte_with_update: true,
        storage_types: StorageTypes::POSTGRESQL,
        schema_mutations: SchemaMutations::POSTGRESQL,
        select_for_update: true,
        auto_increment: true,
        bigdecimal_implemented: false,

        // PostgreSQL has native date/time types
        native_timestamp: true,
        native_date: true,
        native_time: true,
        native_datetime: true,

        // PostgreSQL has native NUMERIC type with arbitrary precision
        native_decimal: true,
        decimal_arbitrary_precision: true,

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

        // MySQL has native date/time types
        native_timestamp: true,
        native_date: true,
        native_time: true,
        native_datetime: true,

        // MySQL has DECIMAL type but requires fixed precision/scale upfront
        native_decimal: true,
        decimal_arbitrary_precision: false,
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

        // DynamoDB does not have native date/time types
        native_timestamp: false,
        native_date: false,
        native_time: false,
        native_datetime: false,

        // DynamoDB does not have native decimal types
        native_decimal: false,
        decimal_arbitrary_precision: false,
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

    pub const POSTGRESQL: StorageTypes = StorageTypes {
        default_string_type: db::Type::Text,

        // The maximum n you can specify is 10 485 760 characters. Attempts to
        // declare varchar with a larger typmod will be rejected at
        // table‐creation time.
        varchar: Some(10_485_760),

        default_uuid_type: db::Type::Uuid,

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

    pub const DYNAMODB: StorageTypes = StorageTypes {
        default_string_type: db::Type::Text,

        // DynamoDB does not support varchar types
        varchar: None,

        default_uuid_type: db::Type::Text,

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
    pub const SQLITE: Self = Self {
        alter_column_type: false,
        alter_column_properties_atomic: false,
    };

    pub const POSTGRESQL: Self = Self {
        alter_column_type: true,
        alter_column_properties_atomic: false,
    };

    pub const MYSQL: Self = Self {
        alter_column_type: true,
        alter_column_properties_atomic: true,
    };

    // DynamoDB migrations are currently not supported.
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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("native_varchar is true but storage_types.varchar is None"));
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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("native_varchar is false but storage_types.varchar is Some"));
    }
}
