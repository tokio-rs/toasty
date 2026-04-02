// Models with unique names so we can identify them among all the other models
// in the test binary.

#[derive(Debug, toasty::Model)]
struct DiscoverTestAlphaModel {
    #[key]
    id: u64,
    name: String,
}

#[derive(Debug, toasty::Embed)]
struct DiscoverTestBetaEmbed {
    value: String,
}

#[test]
fn discover_finds_all_in_current_crate() {
    let mut builder = toasty::Db::builder();
    builder.models(toasty::models!(crate::*));

    let schema = builder.build_app_schema().unwrap();
    let names: Vec<String> = schema.models().map(|m| m.name().to_string()).collect();

    assert!(
        names.contains(&"DiscoverTestAlphaModel".to_string()),
        "expected DiscoverTestAlphaModel in discovered models, got: {names:?}"
    );
    assert!(
        names.contains(&"DiscoverTestBetaEmbed".to_string()),
        "expected DiscoverTestBetaEmbed in discovered models, got: {names:?}"
    );
}

#[test]
fn discover_finds_all_in_third_party_crate() {
    let mut builder = toasty::Db::builder();
    builder.models(toasty::models!(tests::*));

    let schema = builder.build_app_schema().unwrap();
    let names: Vec<String> = schema.models().map(|m| m.name().to_string()).collect();

    assert!(
        names.contains(&"DiscoverTestAlphaModel".to_string()),
        "expected DiscoverTestAlphaModel in discovered models, got: {names:?}"
    );
    assert!(
        names.contains(&"DiscoverTestBetaEmbed".to_string()),
        "expected DiscoverTestBetaEmbed in discovered models, got: {names:?}"
    );
}
