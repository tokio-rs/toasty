use std::{rc::Rc, sync::Arc};

use tests::{models, tests, DbTest};
use toasty::stmt::Id;

macro_rules! def_num_ty_tests {
    (
        $( $t:ty => $test_values:expr => $test_name:ident; )*
    ) => {
        $(
            #[allow(dead_code)]
            async fn $test_name(test: &mut DbTest) {
                #[derive(Debug, toasty::Model)]
                #[allow(dead_code)]
                struct Foo {
                    #[key]
                    #[auto]
                    id: Id<Self>,
                    val: $t,
                }

                let db = test.setup_db(models!(Foo)).await;
                let mut test_values: Vec<$t> = $test_values.to_vec();

                // Filter test values based on database capabilities for unsigned integers
                // Unsigned types have MIN == 0, signed types have MIN < 0
                if <$t>::MIN == 0 {
                    if let Some(max_unsigned) = test.capability().storage_types.max_unsigned_integer {
                        test_values.retain(|&val| {
                            let val_as_u64 = val as u64;
                            val_as_u64 <= max_unsigned
                        });
                    }
                }

                // Test 1: All test values round-trip
                for &val in &test_values {
                    let created = Foo::create().val(val).exec(&db).await.unwrap();
                    let read = Foo::get_by_id(&db, &created.id).await.unwrap();
                    assert_eq!(read.val, val, "Round-trip failed for: {}", val);

                    // Raw storage verification - verify the value is stored correctly at the database level
                    let mut filter = std::collections::HashMap::new();
                    filter.insert("id".to_string(), toasty_core::stmt::Value::from(created.id));

                    let raw_stored_value = test.get_raw_column_value::<$t>("foos", "val", filter).await
                        .unwrap_or_else(|e| panic!("Raw storage verification failed for {} value {}: {}", stringify!($t), val, e));

                    assert_eq!(
                        raw_stored_value, val,
                        "Raw storage verification failed for {}: expected {}, got {}",
                        stringify!($t), val, raw_stored_value
                    );
                }

                // Test 2: Multiple records with different values
                let mut created_records = Vec::new();
                for &val in &test_values {
                    let created = Foo::create().val(val).exec(&db).await.unwrap();
                    created_records.push((created.id, val));
                }

                for (id, expected_val) in created_records {
                    let read = Foo::get_by_id(&db, &id).await.unwrap();
                    assert_eq!(read.val, expected_val, "Multiple records test failed for: {}", expected_val);
                }

                // Test 3: Update chain
                if !test_values.is_empty() {
                    let mut record = Foo::create().val(test_values[0]).exec(&db).await.unwrap();
                    for &val in &test_values {
                        record.update().val(val).exec(&db).await.unwrap();
                        let read = Foo::get_by_id(&db, &record.id).await.unwrap();
                        assert_eq!(read.val, val, "Update chain failed for: {}", val);
                        record.val = val;
                    }
                }
            }
        )*
    };
}

def_num_ty_tests!(
    i8 => &[i8::MIN, -100, -1, 0, 1, 63, 100, i8::MAX] => ty_i8;
    i16 => &[i16::MIN, -10000, -1, 0, 1, 10000, 16383, i16::MAX] => ty_i16;
    i32 => &[i32::MIN, -1000000, -1, 0, 1, 1000000, 1073741823, i32::MAX] => ty_i32;
    i64 => &[i64::MIN, -1000000000000, -1, 0, 1, 1000000000000, 4611686018427387903, i64::MAX] => ty_i64;
    isize => &[isize::MIN, -1000000000000, -1, 0, 1, 1000000000000, 4611686018427387903, isize::MAX] => ty_isize;
    u8 => &[u8::MIN, 0, 1, 100, 127, 200, u8::MAX] => ty_u8;
    u16 => &[u16::MIN, 0, 1, 10000, 32767, 50000, u16::MAX] => ty_u16;
    u32 => &[u32::MIN, 0, 1, 1000000, 2147483647, 2000000000, u32::MAX] => ty_u32;
    u64 => &[u64::MIN, 0, 1, 1000000000000, 9223372036854775807, 10000000000000000000, u64::MAX] => ty_u64;
    usize => &[usize::MIN, 0, 1, 1000000000000, 9223372036854775807, 10000000000000000000, usize::MAX] => ty_usize;
);

#[allow(dead_code)]
async fn ty_str(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: String,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values: Vec<_> = [
        gen_string(0, "empty"),
        gen_string(10, "ascii"),
        // 20 * 4 bytes = 80 bytes (well under MySQL's 191-byte limit)
        gen_string(20, "unicode"),
        gen_string(100, "mixed"),
        gen_string(1_000, "ascii"),
        gen_string(10_000, "mixed"),
        // ~100KB - well under DynamoDB's 400KB limit
        gen_string(100_000, "ascii"),
        gen_string(20, "newlines"),
        gen_string(100, "spaces"),
    ]
    .into_iter()
    .filter(
        |value| match test.capability().default_string_max_length() {
            Some(max_len) => max_len >= value.len() as _,
            None => true,
        },
    )
    .collect();

    // Test 1: All test values round-trip
    for val in &test_values {
        let created = Foo::create().val((*val).clone()).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val);
    }

    // Test 3: Update chain
    let mut record = Foo::create().val(&test_values[0]).exec(&db).await.unwrap();

    for val in &test_values {
        record.update().val(val).exec(&db).await.unwrap();

        let read = Foo::get_by_id(&db, &record.id).await.unwrap();

        assert_eq!(read.val, *val,);
    }
}

// Helper function to generate a test string with specific characteristics
fn gen_string(length: usize, pattern: &str) -> String {
    match pattern {
        "empty" => String::new(),
        "ascii" => "a".repeat(length),
        "unicode" => "🦀".repeat(length),
        "mixed" => "test ".repeat(length / 5), // ~5 chars per repeat
        "newlines" => "line\n".repeat(length / 5),
        "spaces" => " ".repeat(length),
        _ => pattern.repeat(length / pattern.len().max(1)),
    }
}

async fn ty_uuid(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: uuid::Uuid,
    }

    let db = test.setup_db(models!(Foo)).await;
    for _ in 0..16 {
        let val = uuid::Uuid::new_v4();
        let created = Foo::create().val(val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, val);
    }
}

async fn ty_smart_ptrs(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        arced: Arc<i32>,
        rced: Rc<i32>,
        boxed: Box<i32>,
    }

    let db = test.setup_db(models!(Foo)).await;

    let created = Foo::create()
        .arced(1i32)
        .rced(2i32)
        .boxed(3i32)
        .exec(&db)
        .await
        .unwrap();

    let read = Foo::get_by_id(&db, &created.id).await.unwrap();
    assert_eq!(created.id, read.id);
    assert_eq!(created.arced, read.arced);
    assert_eq!(created.rced, read.rced);
    assert_eq!(created.boxed, read.boxed);
}

tests!(
    ty_i8,
    ty_i16,
    ty_i32,
    ty_i64,
    ty_isize,
    ty_u8,
    ty_u16,
    ty_u32,
    ty_u64,
    ty_usize,
    ty_str,
    ty_uuid,
    ty_smart_ptrs
);
