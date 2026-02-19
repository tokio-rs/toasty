use crate::prelude::*;

use std::{rc::Rc, sync::Arc};
use toasty::{schema::db, stmt::Id};
use toasty_core::{
    driver::Operation,
    stmt::{ExprSet, InsertTarget, Statement},
};

/// Macro to generate the common test body for numeric types
macro_rules! num_ty_test_body {
    ($test:expr, $ty:ty, $test_values:expr) => {{
        #[derive(Debug, toasty::Model)]
        #[allow(dead_code)]
        struct Foo {
            #[key]
            #[auto]
            id: Id<Self>,
            val: $ty,
        }

        let test = $test;
        let db = test.setup_db(models!(Foo)).await;
        let mut test_values: Vec<$ty> = (*$test_values).to_vec();

        // Filter test values based on database capabilities for unsigned integers
        // Unsigned types have MIN == 0, signed types have MIN < 0
        if <$ty>::MIN == 0 {
            if let Some(max_unsigned) = test.capability().storage_types.max_unsigned_integer {
                test_values.retain(|&val| {
                    let val_as_u64 = val as u64;
                    val_as_u64 <= max_unsigned
                });
            }
        }

        // Clear setup operations
        test.log().clear();

        // Test 1: All test values round-trip
        for &val in &test_values {
            let created = Foo::create().val(val).exec(&db).await?;

            // Verify the INSERT operation stored the correct value
            let (op, _resp) = test.log().pop();
            assert_struct!(op, Operation::QuerySql(_ {
                stmt: Statement::Insert(_ {
                    target: InsertTarget::Table(_ {
                        table: == table_id(&db, "foos"),
                        columns: == columns(&db, "foos", &["id", "val"]),
                        ..
                    }),
                    source.body: ExprSet::Values(_ {
                        rows: [=~ (Any, val)],
                        ..
                    }),
                    ..
                }),
                ..
            }));

            let read = Foo::get_by_id(&db, &created.id).await?;
            assert_eq!(read.val, val, "Round-trip failed for: {}", val);

            // Clear the read operation
            test.log().clear();
        }

        // Test 2: Multiple records with different values
        let mut created_records = Vec::new();
        for &val in &test_values {
            let created = Foo::create().val(val).exec(&db).await?;
            created_records.push((created.id, val));
            test.log().clear();
        }

        for (id, expected_val) in created_records {
            let read = Foo::get_by_id(&db, &id).await?;
            assert_eq!(
                read.val, expected_val,
                "Multiple records test failed for: {}",
                expected_val
            );
            test.log().clear();
        }

        // Test 3: Update chain
        if !test_values.is_empty() {
            let mut record = Foo::create().val(test_values[0]).exec(&db).await?;
            test.log().clear();

            for &val in &test_values {
                record.update().val(val).exec(&db).await?;

                // Verify the UPDATE operation sent the correct value
                let (op, _resp) = test.log().pop();
                if test.capability().sql {
                    assert_struct!(op, Operation::QuerySql(_ {
                        stmt: Statement::Update(_ {
                            assignments: #{ 1: _ { expr: _, .. }},
                            ..
                        }),
                        ..
                    }));
                } else {
                    assert_struct!(op, Operation::UpdateByKey(_ {
                        assignments: #{ 1: _ { expr: _, .. }},
                        ..
                    }));
                }

                let read = Foo::get_by_id(&db, &record.id).await?;
                assert_eq!(read.val, val, "Update chain failed for: {}", val);
                record.val = val;

                test.log().clear();
            }
        }
        Ok(())
    }};
}

#[driver_test]
pub async fn ty_i8(test: &mut Test) -> Result<()> {
    num_ty_test_body!(test, i8, &[i8::MIN, -100, -1, 0, 1, 63, 100, i8::MAX])
}

#[driver_test]
pub async fn ty_i16(test: &mut Test) -> Result<()> {
    num_ty_test_body!(
        test,
        i16,
        &[i16::MIN, -10000, -1, 0, 1, 10000, 16383, i16::MAX]
    )
}

#[driver_test]
pub async fn ty_i32(test: &mut Test) -> Result<()> {
    num_ty_test_body!(
        test,
        i32,
        &[i32::MIN, -1000000, -1, 0, 1, 1000000, 1073741823, i32::MAX]
    )
}

#[driver_test]
pub async fn ty_i64(test: &mut Test) -> Result<()> {
    num_ty_test_body!(
        test,
        i64,
        &[
            i64::MIN,
            -1000000000000,
            -1,
            0,
            1,
            1000000000000,
            4611686018427387903,
            i64::MAX
        ]
    )
}

#[driver_test]
pub async fn ty_isize(test: &mut Test) -> Result<()> {
    num_ty_test_body!(
        test,
        isize,
        &[
            isize::MIN,
            -1000000000000,
            -1,
            0,
            1,
            1000000000000,
            4611686018427387903,
            isize::MAX
        ]
    )
}

#[driver_test]
pub async fn ty_u8(test: &mut Test) -> Result<()> {
    num_ty_test_body!(test, u8, &[u8::MIN, 0, 1, 100, 127, 200, u8::MAX])
}

#[driver_test]
pub async fn ty_u16(test: &mut Test) -> Result<()> {
    num_ty_test_body!(test, u16, &[u16::MIN, 0, 1, 10000, 32767, 50000, u16::MAX])
}

#[driver_test]
pub async fn ty_u32(test: &mut Test) -> Result<()> {
    num_ty_test_body!(
        test,
        u32,
        &[u32::MIN, 0, 1, 1000000, 2147483647, 2000000000, u32::MAX]
    )
}

#[driver_test]
pub async fn ty_u64(test: &mut Test) -> Result<()> {
    num_ty_test_body!(
        test,
        u64,
        &[
            u64::MIN,
            0,
            1,
            1000000000000,
            9223372036854775807,
            10000000000000000000,
            u64::MAX
        ]
    )
}

#[driver_test]
pub async fn ty_usize(test: &mut Test) -> Result<()> {
    num_ty_test_body!(
        test,
        usize,
        &[
            usize::MIN,
            0,
            1,
            1000000000000,
            9223372036854775807,
            10000000000000000000,
            usize::MAX
        ]
    )
}

#[driver_test]
pub async fn ty_str(test: &mut Test) -> Result<()> {
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

    // Clear setup operations
    test.log().clear();

    // Test 1: All test values round-trip
    for val in &test_values {
        let created = Foo::create().val((*val).clone()).exec(&db).await?;

        // Verify the INSERT operation stored the string value
        let (op, _resp) = test.log().pop();
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Insert(_ {
                target: InsertTarget::Table(_ {
                    table: == table_id(&db, "foos"),
                    columns: == columns(&db, "foos", &["id", "val"]),
                    ..
                }),
                source.body: ExprSet::Values(_ {
                    rows: [=~ (Any, val)],
                    ..
                }),
                ..
            }),
            ..
        }));

        let read = Foo::get_by_id(&db, &created.id).await?;
        assert_eq!(read.val, *val);

        test.log().clear();
    }

    // Test 2: Update chain
    let mut record = Foo::create().val(&test_values[0]).exec(&db).await?;
    test.log().clear();

    for val in &test_values {
        record.update().val(val).exec(&db).await?;

        // Verify the UPDATE operation sent the string value
        let (op, _resp) = test.log().pop();
        if test.capability().sql {
            assert_struct!(op, Operation::QuerySql(_ {
                stmt: Statement::Update(_ {
                    assignments: #{ 1: _ { expr: _, .. }},
                    ..
                }),
                ..
            }));
        } else {
            assert_struct!(op, Operation::UpdateByKey(_ {
                assignments: #{ 1: _ { expr: _, .. }},
                ..
            }));
        }

        let read = Foo::get_by_id(&db, &record.id).await?;
        assert_eq!(read.val, *val);

        test.log().clear();
    }
    Ok(())
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

#[driver_test]
pub async fn ty_uuid(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: uuid::Uuid,
    }

    let db = test.setup_db(models!(Foo)).await;

    // Clear setup operations
    test.log().clear();

    for _ in 0..16 {
        let val = uuid::Uuid::new_v4();
        let created = Foo::create().val(val).exec(&db).await?;

        // Verify the INSERT operation - UUID should be stored in its native format
        let (op, _resp) = test.log().pop();
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Insert(_ {
                target: InsertTarget::Table(_ {
                    table: == table_id(&db, "foos"),
                    columns: == columns(&db, "foos", &["id", "val"]),
                    ..
                }),
                ..
            }),
            ..
        }));

        match &test.capability().storage_types.default_uuid_type {
            db::Type::Uuid => {
                assert_struct!(op, Operation::QuerySql(_ {
                    stmt: Statement::Insert(_ {
                        source.body: ExprSet::Values(_ {
                            rows: [=~ (Any, val)],
                            ..
                        }),
                        ..
                    }),
                    ..
                }));
            }
            db::Type::Blob => {
                assert_struct!(op, Operation::QuerySql(_ {
                    stmt: Statement::Insert(_ {
                        source.body: ExprSet::Values(_ {
                            rows: [=~ (Any, val.as_bytes())],
                            ..
                        }),
                        ..
                    }),
                    ..
                }));
            }
            db::Type::Text | db::Type::VarChar(..) => {
                assert_struct!(op, Operation::QuerySql(_ {
                    stmt: Statement::Insert(_ {
                        source.body: ExprSet::Values(_ {
                            rows: [=~ (Any, val.to_string())],
                            ..
                        }),
                        ..
                    }),
                    ..
                }));
            }
            ty => todo!("ty={ty:#?}"),
        }

        let read = Foo::get_by_id(&db, &created.id).await?;
        assert_eq!(read.val, val);

        test.log().clear();
    }
    Ok(())
}

#[driver_test]
pub async fn ty_smart_ptrs(test: &mut Test) -> Result<()> {
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

    // Clear setup operations
    test.log().clear();

    let created = Foo::create()
        .arced(1i32)
        .rced(2i32)
        .boxed(3i32)
        .exec(&db)
        .await?;

    // Verify the INSERT operation stored the unwrapped values
    let (op, _resp) = test.log().pop();
    assert_struct!(op, Operation::QuerySql(_ {
        stmt: Statement::Insert(_ {
            target: InsertTarget::Table(_ {
                table: == table_id(&db, "foos"),
                columns: == columns(&db, "foos", &["id", "arced", "rced", "boxed"]),
                ..
            }),
            source.body: ExprSet::Values(_ {
                rows: [=~ (Any, Any, Any, Any)],
                ..
            }),
            ..
        }),
        ..
    }));

    let read = Foo::get_by_id(&db, &created.id).await?;
    assert_eq!(created.id, read.id);
    assert_eq!(created.arced, read.arced);
    assert_eq!(created.rced, read.rced);
    assert_eq!(created.boxed, read.boxed);

    test.log().clear();
    Ok(())
}
