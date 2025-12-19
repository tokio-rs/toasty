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
fn hello_world() {
    let setup = SqliteSetup;
    let suite = toasty_driver_integration_suite::IntegrationSuite::new(setup);
    suite.run();
}
