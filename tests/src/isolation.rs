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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolation_generates_unique_prefixes() {
        let isolation1 = TestIsolation::new();
        let isolation2 = TestIsolation::new();

        let prefix1 = isolation1.table_prefix();
        let prefix2 = isolation2.table_prefix();

        assert_ne!(prefix1, prefix2);
        assert!(prefix1.starts_with("test_"));
        assert!(prefix2.starts_with("test_"));
    }

    #[test]
    fn test_isolation_owns_table() {
        let isolation = TestIsolation::new();
        let prefix = isolation.table_prefix();
        let table_name = format!("{}users", prefix);

        assert!(isolation.owns_table(&table_name));

        // Different isolation instance shouldn't own the table
        let other_isolation = TestIsolation::new();
        assert!(!other_isolation.owns_table(&table_name));
    }

    #[test]
    fn test_table_prefix_format() {
        let isolation = TestIsolation::new();
        let prefix = isolation.table_prefix();

        // Should match format: test_{process_id}_{counter}_
        let parts: Vec<&str> = prefix.split('_').collect();
        assert_eq!(parts.len(), 4); // ["test", "{process_id}", "{counter}", ""]
        assert_eq!(parts[0], "test");
        assert!(parts[1].parse::<u32>().is_ok()); // process_id should be numeric
        assert!(parts[2].parse::<u32>().is_ok()); // counter should be numeric
        assert_eq!(parts[3], ""); // trailing underscore creates empty part
    }
}
