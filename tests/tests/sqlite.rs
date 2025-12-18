#[test]
#[ignore]
fn hello_world() {
    let driver = toasty_driver_sqlite::Sqlite::in_memory();
    let suite = toasty_driver_integration_suite::IntegrationSuite::new(driver);
    suite.run();
}
