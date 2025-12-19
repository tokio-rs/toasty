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

    /// Run a single test
    pub fn run_test(&self, name: &str) {
        todo!()
    }
}
