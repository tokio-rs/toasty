#![cfg(any(feature = "mysql", feature = "mariadb"))]

use std::{net::TcpListener, time::Duration};
use toasty_core::driver::{Capability, Dialect, Driver};
use toasty_core::schema::db::Type;

#[cfg(feature = "mysql")]
#[test]
fn mysql_constructor_is_offline() {
    let cap: &'static Capability = {
        let url = "mysql://localhost:1/test";
        let driver = toasty_driver_mysql::MySQL::new(url).unwrap();
        assert_eq!(driver.url(), url);
        driver.capability()
    };
    assert_eq!(cap.sql, Some(Dialect::Mysql));
    assert_eq!(cap.storage_types.default_uuid_type, Type::VarChar(36));
    assert!(!cap.returning_from_insert);
    assert!(!cap.returning_from_update);

    let error = toasty_driver_mysql::MySQL::new("mariadb://localhost/test").unwrap_err();
    assert!(error.is_invalid_connection_url());
}

#[cfg(feature = "mariadb")]
#[test]
fn mariadb_constructor_is_offline() {
    let cap: &'static Capability = {
        let url = "mariadb://localhost:1/test";
        let driver = toasty_driver_mariadb::MariaDb::new(url).unwrap();
        assert_eq!(driver.url(), url);
        driver.capability()
    };
    assert_eq!(cap.sql, Some(Dialect::MariaDb));
    assert_eq!(cap.storage_types.default_uuid_type, Type::Uuid);
    assert!(cap.returning_from_insert);
    assert!(!cap.returning_from_update);

    for url in [
        "mysql://localhost/test",
        "mariadb://localhost",
        "mariadb:///test",
    ] {
        let error = toasty_driver_mariadb::MariaDb::new(url).unwrap_err();
        assert!(error.is_invalid_connection_url(), "{url}: {error}");
    }
}

#[tokio::test]
async fn url_selects_driver_and_requires_its_feature() {
    for (url, enabled, dialect) in [
        (
            "mysql://localhost:1/test",
            cfg!(feature = "mysql"),
            Dialect::Mysql,
        ),
        (
            "mariadb://localhost:1/test",
            cfg!(feature = "mariadb"),
            Dialect::MariaDb,
        ),
    ] {
        let result = toasty::db::Connect::new(url).await;
        if enabled {
            let driver = result.unwrap();
            assert_eq!(driver.url(), url);
            assert_eq!(driver.capability().sql, Some(dialect));
        } else {
            assert!(result.unwrap_err().is_unsupported_feature());
        }
    }
}

#[tokio::test]
async fn pool_create_timeout_covers_server_handshake() {
    for (scheme, enabled) in [
        ("mysql", cfg!(feature = "mysql")),
        ("mariadb", cfg!(feature = "mariadb")),
    ] {
        if !enabled {
            continue;
        }

        // TCP connects, but this listener never sends a database handshake.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("{scheme}://{}/test", listener.local_addr().unwrap());
        let mut builder = toasty::Db::builder();
        builder.pool_create_timeout(Some(Duration::from_millis(50)));
        let result = tokio::time::timeout(Duration::from_secs(2), builder.connect(&url))
            .await
            .expect("connection setup bypassed pool_create_timeout");
        assert!(result.unwrap_err().is_connection_pool());
    }
}
