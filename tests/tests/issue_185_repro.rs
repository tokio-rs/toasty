// Reproduction for Issue #185: No compile-time validation of storage type vs Rust type
//
// This test demonstrates that Toasty currently allows incompatible storage types
// to be specified for fields without compile-time validation.

use toasty::stmt::Id;

// This model compiles successfully even though the storage type
// doesn't match the Rust field type
#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    // ✓ VALID: String field with varchar storage - semantically correct
    #[column(type = varchar(50))]
    name: String,

    // ✗ INVALID: Integer field with varchar storage - semantically incorrect
    // This compiles without error but doesn't make sense conceptually
    #[column(type = varchar(10))]
    age: i32,

    // ✗ INVALID: i64 field with varchar storage - semantically incorrect
    #[column(type = varchar(20))]
    score: i64,
}

#[test]
fn issue_185_incompatible_types_compile_without_error() {
    // If this test compiles, it proves the issue exists.
    //
    // Ideally, specifying `varchar` storage for integer fields (age, score)
    // should produce a compile-time error like:
    //
    //   error: incompatible storage type
    //     --> tests/tests/issue_185_repro.rs:23:7
    //      |
    //   23 |     #[column(type = varchar(10))]
    //      |       ^^^^^^^^^^^^^^^^^^^^^^^^^^^ storage type `varchar` is incompatible with field type `i32`
    //      |
    //      = note: varchar storage is only valid for String fields
    //      = help: remove the column attribute or use an appropriate storage type
}
