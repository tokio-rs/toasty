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
fn discover_finds_all_registered_types() {
    let mut builder = toasty::Db::builder();
    builder.discover();

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
