/// Provides the necessary context to each test
pub(crate) trait Context {
    /// The ID type used for this test execution
    type Id;
}
