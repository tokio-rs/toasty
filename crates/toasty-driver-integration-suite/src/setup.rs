use toasty_core::driver::Driver;

#[async_trait::async_trait]
pub trait Setup: Send + Sync + 'static {
    /// Return a new instance of the driver
    fn driver(&self) -> Box<dyn Driver>;

    /// Delete the table with the specified name. This is used by the test
    /// runner to cleanup after itself.
    async fn delete_table(&self, name: &str);

    /// Delete a test table and report cleanup failures to async runners.
    ///
    /// Native test setups can keep implementing [`delete_table`](Self::delete_table);
    /// request-driven runners override this method when cleanup errors must be
    /// returned to a host process.
    async fn try_delete_table(&self, name: &str) -> toasty::Result<()> {
        self.delete_table(name).await;
        Ok(())
    }
}
