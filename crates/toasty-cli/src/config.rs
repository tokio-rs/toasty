use crate::migration::MigrationConfig;

/// Configuration for Toasty CLI operations
#[derive(Debug, Default, Clone)]
pub struct Config {
    /// Migration-related configuration
    pub migration: MigrationConfig,
}

impl Config {
    /// Create a new Config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the migration configuration
    pub fn migration(mut self, migration: MigrationConfig) -> Self {
        self.migration = migration;
        self
    }
}
