use std::sync::Arc;

use toasty::driver::Driver;

use crate::Test;

pub struct IntegrationSuite {
    driver: Arc<dyn Driver>,
}

impl IntegrationSuite {
    pub fn new(driver: impl Driver) -> IntegrationSuite {
        IntegrationSuite {
            driver: Arc::new(driver),
        }
    }

    /// Run the integration suite
    pub fn run(&self) {
        let mut test = Test::new(self.driver.clone());
        test.run(|t| {
            Box::pin(async move {
                crate::tests::one_model_crud::crud_no_fields::id_u64(t).await;
            })
        });
    }
}
