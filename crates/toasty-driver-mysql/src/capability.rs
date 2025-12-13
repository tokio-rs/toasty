use toasty_core::{
    driver::{Capability, StorageTypes},
    schema::db,
};

pub const CAPABILITY: Capability = Capability {
    sql: true,
    cte_with_update: false,
    select_for_update: true,
    primary_key_ne_predicate: true,
    bigdecimal_implemented: true,
    storage_types: StorageTypes {
        default_string_type: db::Type::VarChar(191),

        // Values in VARCHAR columns are variable-length strings. The length can
        // be specified as a value from 0 to 65,535. The effective maximum
        // length of a VARCHAR is subject to the maximum row size (65,535 bytes,
        // which is shared among all columns) and the character set used.
        varchar: Some(65_535),

        // MySQL does not have an inbuilt UUID type. The binary blob type is more
        // difficult to read than Text but likely has better performance characteristics.
        default_uuid_type: db::Type::Binary(16),

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

        // MySQL has native date/time types
        native_timestamp: true,
        native_date: true,
        native_time: true,
        native_datetime: true,

        // MySQL has DECIMAL type but requires fixed precision/scale upfront
        native_decimal: true,
        decimal_arbitrary_precision: false,

        // MySQL supports full u64 range via BIGINT UNSIGNED
        max_unsigned_integer: None,
    },
};
