use toasty_core::{
    driver::{Capability, StorageTypes},
    schema::db,
};

pub const CAPABILITY: Capability = Capability {
    sql: false,
    cte_with_update: false,
    select_for_update: false,
    primary_key_ne_predicate: false,
    bigdecimal_implemented: false,
    storage_types: StorageTypes {
        default_string_type: db::Type::Text,

        // DynamoDB does not support varchar types
        varchar: None,

        default_uuid_type: db::Type::Blob,

        // DynamoDB does not have a native decimal type. Store as TEXT.
        default_decimal_type: db::Type::Text,
        default_bigdecimal_type: db::Type::Text,

        // DynamoDB does not have native date/time types. Store as TEXT (strings).
        default_timestamp_type: db::Type::Text,
        default_zoned_type: db::Type::Text,
        default_date_type: db::Type::Text,
        default_time_type: db::Type::Text,
        default_datetime_type: db::Type::Text,

        // DynamoDB does not have native date/time types
        native_timestamp: false,
        native_date: false,
        native_time: false,
        native_datetime: false,

        // DynamoDB does not have native decimal types
        native_decimal: false,
        decimal_arbitrary_precision: false,

        // DynamoDB supports full u64 range (numbers stored as strings)
        max_unsigned_integer: None,
    },
};
