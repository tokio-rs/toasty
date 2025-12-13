use toasty_core::{
    driver::{Capability, StorageTypes},
    schema::db,
};

pub const CAPABILITY: Capability = Capability {
    sql: true,
    cte_with_update: false,
    select_for_update: false,
    primary_key_ne_predicate: true,
    bigdecimal_implemented: false,
    storage_types: StorageTypes {
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

        // SQLite does not have native date/time types
        native_timestamp: false,
        native_date: false,
        native_time: false,
        native_datetime: false,

        // SQLite does not have native decimal types
        native_decimal: false,
        decimal_arbitrary_precision: false,

        // SQLite INTEGER is a signed 64-bit integer, so unsigned integers
        // are limited to i64::MAX to prevent overflow
        max_unsigned_integer: Some(i64::MAX as u64),
    },
};
