use tests::*;

use toasty::stmt::Id;

tests!(ty_i32, ty_i64, ty_str);

macro_rules! def_num_ty_tests {
    (
        $( $t:ty => $test_values:expr => $test_name:ident; )*
    ) => {
        $(
            #[allow(dead_code)]
            async fn $test_name(s: impl Setup) {
                #[derive(Debug, toasty::Model)]
                #[allow(dead_code)]
                struct Foo {
                    #[key]
                    #[auto]
                    id: Id<Self>,
                    val: $t,
                }

                let db = s.setup(models!(Foo)).await;
                let test_values: &[$t] = $test_values;

                // Test 1: All test values round-trip
                for &val in test_values {
                    let created = Foo::create().val(val).exec(&db).await.unwrap();
                    let read = Foo::get_by_id(&db, &created.id).await.unwrap();
                    assert_eq!(read.val, val, "Round-trip failed for: {}", val);
                }

                // Test 2: Multiple records with different values
                let mut created_records = Vec::new();
                for &val in test_values {
                    let created = Foo::create().val(val).exec(&db).await.unwrap();
                    created_records.push((created.id, val));
                }

                for (id, expected_val) in created_records {
                    let read = Foo::get_by_id(&db, &id).await.unwrap();
                    assert_eq!(read.val, expected_val, "Multiple records test failed for: {}", expected_val);
                }

                // Test 3: Update chain
                let mut record = Foo::create().val(test_values[0]).exec(&db).await.unwrap();
                for &val in test_values {
                    record.update().val(val).exec(&db).await.unwrap();
                    let read = Foo::get_by_id(&db, &record.id).await.unwrap();
                    assert_eq!(read.val, val, "Update chain failed for: {}", val);
                    record.val = val;
                }
            }
        )*
    };
}

def_num_ty_tests!(
    i32 => &[i32::MIN, -1000000, -1, 0, 1, 1000000, i32::MAX] => ty_i32;
    i64 => &[i64::MIN, -1000000000000, -1, 0, 1, 1000000000000, i64::MAX] => ty_i64;
);

#[allow(dead_code)]
async fn ty_str(s: impl Setup) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: String,
    }

    let db = s.setup(models!(Foo)).await;

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
    .filter(|value| match s.capability().default_string_max_length() {
        Some(max_len) => max_len >= value.len() as _,
        None => true,
    })
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
