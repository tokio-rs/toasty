//! Identifying which MySQL-protocol server the driver is connected to.
//!
//! MariaDB shares MySQL's wire protocol and URL scheme, so configuration
//! cannot tell them apart. The driver asks once, on construction.

/// Which server the driver is talking to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flavor {
    MySql,
    MariaDb,
}

/// Identifies the server from the string `SELECT VERSION()` returns.
///
/// MariaDB names itself in that string; anything else is treated as MySQL,
/// which is the capability set every MySQL-protocol server accepts.
pub(crate) fn detect_flavor(version: &str) -> Flavor {
    if version.to_ascii_lowercase().contains("mariadb") {
        Flavor::MariaDb
    } else {
        Flavor::MySql
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mysql_versions() {
        assert_eq!(detect_flavor("8.0.35"), Flavor::MySql);
        assert_eq!(detect_flavor("8.4.2-0ubuntu0.24.04.1"), Flavor::MySql);
        assert_eq!(detect_flavor("5.7.44-log"), Flavor::MySql);
    }

    #[test]
    fn mariadb_versions() {
        assert_eq!(detect_flavor("10.11.6-MariaDB"), Flavor::MariaDb);
        assert_eq!(detect_flavor("11.4.3-MariaDB-ubu2404"), Flavor::MariaDb);
    }

    /// MariaDB 10.x prepends `5.5.5-` for clients predating its version
    /// handshake. The name still appears, so detection is unaffected — but
    /// anything that reads the leading number would see MySQL 5.5.5.
    #[test]
    fn mariadb_replication_version_prefix() {
        assert_eq!(
            detect_flavor("5.5.5-10.11.6-MariaDB-1:10.11.6+maria~ubu2204"),
            Flavor::MariaDb
        );
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(detect_flavor("10.11.6-mariadb"), Flavor::MariaDb);
    }

    /// An unrecognized server gets MySQL's capability, the subset everything
    /// speaking this protocol accepts.
    #[test]
    fn unrecognized_server_is_mysql() {
        assert_eq!(detect_flavor(""), Flavor::MySql);
        assert_eq!(detect_flavor("some-proxy"), Flavor::MySql);
    }
}
