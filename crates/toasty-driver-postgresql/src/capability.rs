use toasty_core::{
    driver::{Capability, StorageTypes},
    schema::db,
};

pub const CAPABILITY: Capability = Capability {
    sql: true,
    cte_with_update: true,
    select_for_update: true,
    primary_key_ne_predicate: true,
    bigdecimal_implemented: false,
    storage_types: StorageTypes {
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

        // PostgreSQL has native date/time types
        native_timestamp: true,
        native_date: true,
        native_time: true,
        native_datetime: true,

        // PostgreSQL has native NUMERIC type with arbitrary precision
        native_decimal: true,
        decimal_arbitrary_precision: true,

        // PostgreSQL BIGINT is signed 64-bit, so unsigned integers are limited
        // to i64::MAX. While NUMERIC could theoretically support larger values,
        // we prefer explicit limits over implicit type switching.
        max_unsigned_integer: Some(i64::MAX as u64),
    },
};
