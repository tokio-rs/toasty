use std::sync::Arc;

use crate::{Setup, Test};

pub struct IntegrationSuite {
    setup: Arc<dyn Setup>,
}

impl IntegrationSuite {
    pub fn new(setup: impl Setup) -> IntegrationSuite {
        IntegrationSuite {
            setup: Arc::new(setup),
        }
    }

    /// Run the integration suite
    pub fn run(&self) {
        let mut test = Test::new(self.setup.clone());
        test.run(async |t| {
            crate::tests::one_model_crud::crud_no_fields::id_u64(t).await;
        });
    }

    /// Run a single test by its path (e.g., "one_model_crud::crud_no_fields::id_u64")
    pub fn run_test(&self, name: &str) {
        let test_fn =
            crate::registry::find_test(name).unwrap_or_else(|| panic!("Test '{}' not found", name));

        let mut test = Test::new(self.setup.clone());
        test.run(async |t| {
            (test_fn.func)(t).await;
        });
    }
}
