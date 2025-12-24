use std::sync::Arc;

use crate::Setup;

pub struct IntegrationSuite {
    setup: Arc<dyn Setup>,
}

impl IntegrationSuite {
    pub fn new(setup: impl Setup) -> IntegrationSuite {
        IntegrationSuite {
            setup: Arc::new(setup),
        }
    }
}
