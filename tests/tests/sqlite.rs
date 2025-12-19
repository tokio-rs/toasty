struct SqliteSetup;

#[async_trait::async_trait]
impl toasty_driver_integration_suite::Setup for SqliteSetup {
    fn driver(&self) -> Box<dyn toasty::driver::Driver> {
        Box::new(toasty_driver_sqlite::Sqlite::in_memory())
    }

    async fn delete_table(&self, _name: &str) {
        // There is no need to delete anything since the driver operates in-memory
    }
}

#[test]
fn run_single_test() {
    let setup = SqliteSetup;
    let suite = toasty_driver_integration_suite::IntegrationSuite::new(setup);

    // Run a specific test by its path
    suite.run_test("one_model_crud::crud_no_fields::id_u64");
}

#[test]
fn run_another_single_test() {
    let setup = SqliteSetup;
    let suite = toasty_driver_integration_suite::IntegrationSuite::new(setup);

    // Run a different test
    suite.run_test("one_model_crud::crud_one_string::id_u64");
}
