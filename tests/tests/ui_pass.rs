#[test]
fn ui_pass() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui-pass/*.rs");
}
