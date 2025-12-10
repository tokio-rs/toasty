use tests::{models, tests, DbTest};
use toasty::stmt::Id;

async fn invalid_u32_with_text_type(test: &mut DbTest) {
    #[derive(toasty::Model)]
    #[allow(dead_code)] // Fields are used by schema validation system
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        // Invalid: u32 field with text storage (type mismatch)
        #[column(type = text)]
        name: u32,
    }

    let result = test.try_setup_db(models!(User)).await;
    assert!(
        result.is_err(),
        "Expected schema validation to fail for u32 field with text storage"
    );

    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("Invalid column type 'TEXT' for field 'name' of type 'u32'"));
    assert!(error_message.contains("u32 fields are compatible with"));
}

async fn invalid_string_with_boolean_type(test: &mut DbTest) {
    #[derive(toasty::Model)]
    #[allow(dead_code)] // Fields are used by schema validation system
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        // Invalid: String field with boolean storage (type mismatch)
        #[column(type = boolean)]
        is_active: String,
    }

    let result = test.try_setup_db(models!(User)).await;
    assert!(
        result.is_err(),
        "Expected schema validation to fail for String field with boolean storage"
    );

    let error_message = result.unwrap_err().to_string();
    assert!(error_message
        .contains("Invalid column type 'BOOLEAN' for field 'is_active' of type 'String'"));
    assert!(error_message.contains("String fields are compatible with"));
}

async fn invalid_i64_with_boolean_storage(test: &mut DbTest) {
    #[derive(toasty::Model)]
    #[allow(dead_code)] // Fields are used by schema validation system
    struct Counter {
        #[key]
        #[auto]
        id: Id<Self>,

        // Invalid: i64 field with boolean storage (type mismatch)
        #[column(type = boolean)]
        count: i64,
    }

    let result = test.try_setup_db(models!(Counter)).await;
    assert!(
        result.is_err(),
        "Expected schema validation to fail for i64 field with boolean storage"
    );

    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("Invalid column type 'BOOLEAN' for field 'count' of type 'i64'"));
    assert!(error_message.contains("i64 fields are compatible with"));
}

async fn invalid_bool_with_text_storage(test: &mut DbTest) {
    #[derive(toasty::Model)]
    #[allow(dead_code)] // Fields are used by schema validation system
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        // Invalid: bool field with text storage (type mismatch)
        #[column(type = text)]
        is_active: bool,
    }

    let result = test.try_setup_db(models!(User)).await;
    assert!(
        result.is_err(),
        "Expected schema validation to fail for bool field with text storage"
    );

    let error_message = result.unwrap_err().to_string();
    assert!(
        error_message.contains("Invalid column type 'TEXT' for field 'is_active' of type 'bool'")
    );
    assert!(error_message.contains("bool fields are compatible with"));
}

async fn valid_compatible_types(test: &mut DbTest) {
    #[derive(toasty::Model)]
    #[allow(dead_code)] // Fields are used by schema validation system
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        // Valid: String with text storage
        #[column(type = text)]
        name: String,

        // Valid: bool with boolean storage
        #[column(type = boolean)]
        is_active: bool,

        // Valid: i32 with default integer storage
        age: i32,

        // Valid: u64 with default unsigned integer storage
        count: u64,
    }

    // This should succeed - all types are compatible
    let result = test.try_setup_db(models!(User)).await;
    assert!(
        result.is_ok(),
        "Expected valid type combinations to succeed: {:?}",
        result
    );
}

async fn type_alias_detection(test: &mut DbTest) {
    // This test demonstrates that schema-time validation catches type aliases
    // that would fool macro-time validation

    type String = u32; // Shadow the built-in String type

    #[derive(toasty::Model)]
    #[allow(dead_code)] // Fields are used by schema validation system
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        // This looks like String to the macro, but is actually u32
        // Schema validation should detect this mismatch
        #[column(type = text)]
        name: String, // This is u32, not std::String
    }

    let result = test.try_setup_db(models!(User)).await;

    // We expect this to return an error due to schema validation
    assert!(
        result.is_err(),
        "Expected schema validation to catch type alias mismatch"
    );

    let error_message = result.unwrap_err().to_string();
    // The error should reflect that it's a u32 field, not String field
    assert!(error_message.contains("Invalid column type 'TEXT' for field 'name' of type 'u32'"));
    assert!(error_message.contains("u32 fields are compatible with: INTEGER, UNSIGNED_INTEGER"));
}

// Jiff date/time type validation tests
async fn valid_jiff_timestamp_text(test: &mut DbTest) {
    #[derive(toasty::Model)]
    #[allow(dead_code)]
    struct Event {
        #[key]
        #[auto]
        id: Id<Self>,

        // Valid: Timestamp with text storage (works on all databases)
        #[column(type = text)]
        created_at: jiff::Timestamp,
    }

    let result = test.try_setup_db(models!(Event)).await;
    assert!(
        result.is_ok(),
        "Expected Timestamp with text storage to succeed: {:?}",
        result
    );
}

async fn valid_jiff_date_native(test: &mut DbTest) {
    #[derive(toasty::Model)]
    #[allow(dead_code)]
    struct Event {
        #[key]
        #[auto]
        id: Id<Self>,

        // Valid: Date with default native storage (varies by database)
        event_date: jiff::civil::Date,
    }

    let result = test.try_setup_db(models!(Event)).await;
    assert!(
        result.is_ok(),
        "Expected Date with native storage to succeed: {:?}",
        result
    );
}

async fn valid_jiff_time_precision(test: &mut DbTest) {
    #[derive(toasty::Model)]
    #[allow(dead_code)]
    struct Schedule {
        #[key]
        #[auto]
        id: Id<Self>,

        // Valid: Time with text storage (works on all databases)
        #[column(type = text)]
        start_time: jiff::civil::Time,
    }

    let result = test.try_setup_db(models!(Schedule)).await;
    assert!(
        result.is_ok(),
        "Expected Time with precision to succeed: {:?}",
        result
    );
}

async fn valid_jiff_datetime_text(test: &mut DbTest) {
    #[derive(toasty::Model)]
    #[allow(dead_code)]
    struct Appointment {
        #[key]
        #[auto]
        id: Id<Self>,

        // Valid: DateTime with text storage (works on all databases)
        #[column(type = text)]
        scheduled_at: jiff::civil::DateTime,
    }

    let result = test.try_setup_db(models!(Appointment)).await;
    assert!(
        result.is_ok(),
        "Expected DateTime with text storage to succeed: {:?}",
        result
    );
}

async fn invalid_jiff_timestamp_with_boolean(test: &mut DbTest) {
    #[derive(toasty::Model)]
    #[allow(dead_code)]
    struct Event {
        #[key]
        #[auto]
        id: Id<Self>,

        // Invalid: Timestamp field with boolean storage (type mismatch)
        #[column(type = boolean)]
        created_at: jiff::Timestamp,
    }

    let result = test.try_setup_db(models!(Event)).await;
    assert!(
        result.is_err(),
        "Expected schema validation to fail for Timestamp field with boolean storage"
    );

    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("Invalid column type"));
    assert!(error_message.contains("field 'created_at' of type 'Timestamp'"));
    assert!(error_message.contains("Timestamp fields are compatible with"));
}

async fn invalid_jiff_date_with_boolean(test: &mut DbTest) {
    #[derive(toasty::Model)]
    #[allow(dead_code)]
    struct Event {
        #[key]
        #[auto]
        id: Id<Self>,

        // Invalid: Date field with boolean storage (type mismatch)
        #[column(type = boolean)]
        event_date: jiff::civil::Date,
    }

    let result = test.try_setup_db(models!(Event)).await;
    assert!(
        result.is_err(),
        "Expected schema validation to fail for Date field with boolean storage"
    );

    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("Invalid column type"));
    assert!(error_message.contains("field 'event_date' of type 'Date'"));
    assert!(error_message.contains("Date fields are compatible with"));
}

async fn valid_jiff_multiple_text_types(test: &mut DbTest) {
    #[derive(toasty::Model)]
    #[allow(dead_code)]
    struct Event {
        #[key]
        #[auto]
        id: Id<Self>,

        // Multiple jiff types with text storage should all work
        #[column(type = text)]
        created_at: jiff::Timestamp,

        #[column(type = text)]
        event_date: jiff::civil::Date,
    }

    let result = test.try_setup_db(models!(Event)).await;
    assert!(
        result.is_ok(),
        "Expected multiple jiff types with text storage to work: {:?}",
        result
    );
}

tests!(
    invalid_u32_with_text_type,
    invalid_string_with_boolean_type,
    invalid_i64_with_boolean_storage,
    invalid_bool_with_text_storage,
    valid_compatible_types,
    type_alias_detection,
    valid_jiff_timestamp_text,
    valid_jiff_date_native,
    valid_jiff_time_precision,
    valid_jiff_datetime_text,
    invalid_jiff_timestamp_with_boolean,
    invalid_jiff_date_with_boolean,
    valid_jiff_multiple_text_types,
);
