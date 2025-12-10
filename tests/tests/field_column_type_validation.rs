use tests::{models, tests, DbTest};
use toasty::stmt::Id;

async fn valid_string_column_types(test: &mut DbTest) {
    #[derive(toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        // Valid: String with text storage
        #[column(type = text)]
        name: String,

        // Valid: String with varchar storage
        #[column(type = varchar(50))]
        email: String,
    }

    // Skip varchar test if not supported by the database
    if test.capability().storage_types.varchar.is_none() {
        return;
    }

    let db = test.setup_db(models!(User)).await;

    // Test basic functionality
    let user = User::create()
        .name("John Doe")
        .email("john@example.com")
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(user.name, "John Doe");
    assert_eq!(user.email, "john@example.com");

    // Test retrieval
    let retrieved = User::get_by_id(&db, &user.id).await.unwrap();
    assert_eq!(retrieved.name, "John Doe");
    assert_eq!(retrieved.email, "john@example.com");
}

async fn valid_integer_column_types(test: &mut DbTest) {
    #[derive(toasty::Model)]
    struct Counter {
        #[key]
        #[auto]
        id: Id<Self>,

        // Valid: i32 with default integer storage
        count: i32,

        // Valid: i32 with default integer storage
        total: i32,

        // Valid: u64 with default unsigned integer storage
        large_value: u64,
    }

    let db = test.setup_db(models!(Counter)).await;

    let counter = Counter::create()
        .count(-12345)
        .total(98765)
        .large_value(1_000_000_000_000_u64)
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(counter.count, -12345);
    assert_eq!(counter.total, 98765);
    assert_eq!(counter.large_value, 1_000_000_000_000_u64);

    // Test retrieval
    let retrieved = Counter::get_by_id(&db, &counter.id).await.unwrap();
    assert_eq!(retrieved.count, -12345);
    assert_eq!(retrieved.total, 98765);
    assert_eq!(retrieved.large_value, 1_000_000_000_000_u64);
}

async fn valid_uuid_column_types(test: &mut DbTest) {
    #[derive(toasty::Model)]
    struct Record {
        #[key]
        #[auto]
        id: Id<Self>,

        // Valid: UUID with text storage
        #[column(type = text)]
        uuid_as_text: uuid::Uuid,

        // Valid: UUID with blob storage
        #[column(type = blob)]
        uuid_as_blob: uuid::Uuid,
    }

    let db = test.setup_db(models!(Record)).await;

    let uuid1 = uuid::Uuid::new_v4();
    let uuid2 = uuid::Uuid::new_v4();

    let record = Record::create()
        .uuid_as_text(uuid1)
        .uuid_as_blob(uuid2)
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(record.uuid_as_text, uuid1);
    assert_eq!(record.uuid_as_blob, uuid2);

    // Test retrieval
    let retrieved = Record::get_by_id(&db, &record.id).await.unwrap();
    assert_eq!(retrieved.uuid_as_text, uuid1);
    assert_eq!(retrieved.uuid_as_blob, uuid2);
}

async fn valid_bool_column_types(test: &mut DbTest) {
    #[derive(toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        // Valid: bool with boolean storage
        #[column(type = boolean)]
        is_active: bool,

        // Valid: bool without explicit column type (should default to boolean)
        is_verified: bool,

        // Valid: Option<bool> with boolean storage
        #[column(type = boolean)]
        is_premium: Option<bool>,
    }

    let db = test.setup_db(models!(User)).await;

    // Test with all bool combinations
    let user = User::create()
        .is_active(true)
        .is_verified(false)
        .is_premium(Some(true))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(user.is_active, true);
    assert_eq!(user.is_verified, false);
    assert_eq!(user.is_premium, Some(true));

    // Test retrieval
    let retrieved = User::get_by_id(&db, &user.id).await.unwrap();
    assert_eq!(retrieved.is_active, true);
    assert_eq!(retrieved.is_verified, false);
    assert_eq!(retrieved.is_premium, Some(true));

    // Test with None optional bool
    let user2 = User::create()
        .is_active(false)
        .is_verified(true)
        .is_premium(None)
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(user2.is_active, false);
    assert_eq!(user2.is_verified, true);
    assert_eq!(user2.is_premium, None);
}

async fn valid_optional_column_types(test: &mut DbTest) {
    #[derive(toasty::Model)]
    #[allow(dead_code)] // Fields are used through generated builder methods
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        name: String,

        // Valid: Option<String> with varchar storage
        #[column(type = varchar(100))]
        bio: Option<String>,

        // Valid: Option<i32> with default integer storage
        age: Option<i32>,
    }

    if test.capability().storage_types.varchar.is_none() {
        return;
    }

    let db = test.setup_db(models!(User)).await;

    // Test with Some values
    let user1 = User::create()
        .name("Alice")
        .bio(Some("Software developer".to_string()))
        .age(Some(30))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(user1.bio, Some("Software developer".to_string()));
    assert_eq!(user1.age, Some(30));

    // Test with None values
    let user2 = User::create()
        .name("Bob")
        .bio(None)
        .age(None)
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(user2.bio, None);
    assert_eq!(user2.age, None);
}

tests!(
    valid_string_column_types,
    valid_integer_column_types,
    valid_uuid_column_types,
    valid_bool_column_types,
    valid_optional_column_types,
);
