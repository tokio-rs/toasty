use std::sync::atomic::{AtomicU32, Ordering};

/// Generates unique table prefixes for test isolation.
///
/// Each test gets a unique prefix in the format: `test_{process_id}_{test_counter}_`
/// This ensures that tests running in parallel (within or across processes) never
/// interfere with each other's database tables.
#[derive(Clone)]
#[allow(dead_code)] // Only used when database features are enabled
pub struct TestIsolation {
    process_id: u32,
    test_counter: u32,
}

// Global counter shared across all tests in this process
#[allow(dead_code)] // Only used when database features are enabled
static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

#[allow(dead_code)] // Only used when database features are enabled
impl TestIsolation {
    /// Create a new test isolation instance with a unique counter.
    pub fn new() -> Self {
        Self {
            process_id: Self::generate_process_id(),
            test_counter: TEST_COUNTER.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// Generate the table prefix for this test.
    pub fn table_prefix(&self) -> String {
        format!("test_{}_{}_", self.process_id, self.test_counter)
    }

    /// Check if a table name belongs to this test isolation instance.
    pub fn owns_table(&self, table_name: &str) -> bool {
        let my_prefix = format!("test_{}_{}_", self.process_id, self.test_counter);
        table_name.starts_with(&my_prefix)
    }

    /// Generate a process ID using the OS process ID.
    fn generate_process_id() -> u32 {
        std::process::id()
    }
}
