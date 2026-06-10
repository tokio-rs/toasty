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
