use super::Error;

/// Error when a migration file has an unsupported format version.
///
/// This occurs when loading a history file or snapshot file whose version
/// number does not match the version expected by this build of Toasty.
#[derive(Debug)]
pub(super) struct UnsupportedMigrationVersion {
    found: u32,
    expected: u32,
}

impl std::error::Error for UnsupportedMigrationVersion {}

impl core::fmt::Display for UnsupportedMigrationVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "unsupported migration version: found {}, expected {}",
            self.found, self.expected
        )
    }
}

impl Error {
    /// Creates an unsupported migration version error.
    ///
    /// Used when a migration file's format version does not match the
    /// version expected by the current build.
    pub fn unsupported_migration_version(found: u32, expected: u32) -> Error {
        Error::from(super::ErrorKind::UnsupportedMigrationVersion(
            UnsupportedMigrationVersion { found, expected },
        ))
    }

    /// Returns `true` if this error is an unsupported migration version error.
    pub fn is_unsupported_migration_version(&self) -> bool {
        matches!(
            self.kind(),
            super::ErrorKind::UnsupportedMigrationVersion(_)
        )
    }
}
