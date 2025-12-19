use toasty::driver::Driver;

#[async_trait::async_trait]
pub trait Setup: Send + Sync + 'static {
    /// Return a new instance of the driver
    fn driver(&self) -> Box<dyn Driver>;

    /// Delete the table with the specified name. This is used by the test
    /// runner to cleanup after itself.
    async fn delete_table(&self, name: &str);
}
