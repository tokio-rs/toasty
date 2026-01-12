use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use toasty_core::schema::db::Schema;

const LOCK_FILE_VERSION: u32 = 1;

/// Lock file containing the current database schema state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    /// Lock file format version
    version: u32,

    /// The database schema
    pub schema: Schema,
}

impl LockFile {
    /// Create a new lock file with the given schema
    pub fn new(schema: Schema) -> Self {
        Self {
            version: LOCK_FILE_VERSION,
            schema,
        }
    }

    /// Load a lock file from a TOML file
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let contents = std::fs::read_to_string(path.as_ref())?;
        contents.parse()
    }

    /// Save the lock file to a TOML file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path.as_ref(), self.to_string())?;
        Ok(())
    }
}

impl FromStr for LockFile {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let lock_file: LockFile = toml::from_str(s)?;

        // Validate version
        if lock_file.version != LOCK_FILE_VERSION {
            bail!(
                "Unsupported lock file version: {}. Expected version {}",
                lock_file.version,
                LOCK_FILE_VERSION
            );
        }

        Ok(lock_file)
    }
}

impl fmt::Display for LockFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let toml_string = toml::to_string_pretty(self)
            .map_err(|_| fmt::Error)?;
        write!(f, "{}", toml_string)
    }
}
