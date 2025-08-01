use crate::Setup;

/// Internal wrapper that manages the Tokio runtime and ensures cleanup happens.
///
/// This is an implementation detail that allows us to:
/// 1. Use #[test] instead of #[tokio::test] for better control
/// 2. Ensure cleanup blocks before the test process exits
/// 3. Keep the existing test API unchanged
pub struct ToastyTest<S: Setup> {
    runtime: tokio::runtime::Runtime,
    setup: Option<S>,
}

impl<S: Setup> ToastyTest<S> {
    /// Create a new ToastyTest with a current-thread runtime.
    pub fn new(setup: S) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");

        Self {
            runtime,
            setup: Some(setup),
        }
    }

    /// Run a test function with the setup, using our managed runtime.
    pub fn run_test<F, Fut>(&mut self, test_fn: F)
    where
        F: FnOnce(S) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let setup = self.setup.take().expect("Setup already consumed");
        self.runtime.block_on(async {
            test_fn(setup).await;
        });
    }
}

impl<S: Setup> Drop for ToastyTest<S> {
    fn drop(&mut self) {
        // If setup is still present, clean it up
        if let Some(setup) = self.setup.take() {
            self.runtime.block_on(async {
                let _ = setup.cleanup_my_tables().await;
            });
        }
    }
}
