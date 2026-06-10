#[test]
fn item_parent_compiles() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/item_parent_basic.rs");
}

#[test]
fn item_parent_requires_deferred() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/item_parent_not_deferred.rs");
}

#[test]
fn item_parent_at_most_one() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/item_parent_multiple.rs");
}

#[test]
fn legacy_item_collection_attribute_rejected() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/legacy_item_collection.rs");
}

// B-corr-1: item-collection children must be created through the parent
// handle (`tenant.users().create()...`); the inherent `User::create()` is
// suppressed so a misuse is a compile error rather than a runtime panic on
// sk encoding mismatch.
#[test]
fn top_level_create_rejected_on_item_parent_child() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/top_level_create.rs");
}

// B-corr-2: even on the relation-handle create-builder, setters for the
// partition field (R2.4: inherited from parent), sort field (R7.1: owned by
// the encoder), and `#[item_parent]` navigation field must not exist. The
// `#[item_parent]` no-setter rule was already in place since B4.7; this test
// pins the partition + sort suppression added by B-corr-2.
#[test]
fn create_builder_setters_rejected_on_item_parent_child() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/create_builder_setters.rs");
}

// B-corr-3: the `create!` macro form must reject the same Toasty-owned
// fields the create-builder does (B-corr-2). The scoped form `create!(in
// tenant.users() { ... })` expands to method calls on the create-builder,
// so suppressing `.sk(...)` and `.account(...)` setters in B-corr-2 already
// closes the macro path. This test pins the behaviour so a future change
// that re-introduces those setters surfaces here too.
#[test]
fn create_macro_rejects_owned_fields_on_item_parent_child() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/create_macro_owned_fields.rs");
}
