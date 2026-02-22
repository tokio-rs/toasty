use crate::migration::MigrationConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Configuration for Toasty CLI operations
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Migration-related configuration
    pub migration: MigrationConfig,
}

impl Config {
    /// Create a new Config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from Toasty.toml in the project root
    pub fn load() -> Result<Self> {
        let path = Path::new("Toasty.toml");
        let contents = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Set the migration configuration
    pub fn migration(mut self, migration: MigrationConfig) -> Self {
        self.migration = migration;
        self
    }
}
