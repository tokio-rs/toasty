use crate::Test;

/// A registered test function
pub struct RegisteredTest {
    /// Full path to the test (e.g., "one_model_crud::crud_no_fields::id_u64")
    pub name: &'static str,

    /// The test function to execute
    pub func: fn(&mut Test) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + '_>>,
}

#[linkme::distributed_slice]
pub static TESTS: [RegisteredTest];

/// Find a test by its path. The path can be either:
/// - Relative to tests module: "one_model_crud::crud_no_fields::id_u64"
/// - Full path: "toasty_driver_integration_suite::tests::one_model_crud::crud_no_fields::id_u64"
pub fn find_test(name: &str) -> Option<&'static RegisteredTest> {
    const PREFIX: &str = "toasty_driver_integration_suite::tests::";

    TESTS.iter().find(|t| {
        // Try exact match first
        if t.name == name {
            return true;
        }

        // Try stripping the prefix from the registered name
        if let Some(short_name) = t.name.strip_prefix(PREFIX) {
            if short_name == name {
                return true;
            }
        }

        false
    })
}

/// Get all registered test names (returns short names relative to tests module)
pub fn all_test_names() -> Vec<&'static str> {
    const PREFIX: &str = "toasty_driver_integration_suite::tests::";

    TESTS
        .iter()
        .map(|t| t.name.strip_prefix(PREFIX).unwrap_or(t.name))
        .collect()
}
