use std::path::PathBuf;
use toasty_core::driver::ConnectionUrl;

#[test]
fn parses_and_validates_scheme() {
    let url = ConnectionUrl::parse("SQLITE::memory:").unwrap();
    assert_eq!(url.scheme(), "SQLITE");
    assert!(url.has_scheme("sqlite"));

    assert!(ConnectionUrl::parse("sqlite").is_err());
    assert!(ConnectionUrl::parse(":memory:").is_err());
    assert!(ConnectionUrl::parse("sql@ite::memory:").is_err());
}

#[test]
fn resolves_file_path_forms() {
    for (url, expected) in [
        ("sqlite:todos.db", "todos.db"),
        ("sqlite://todos.db", "todos.db"),
        ("sqlite://./todos.db", "./todos.db"),
        ("sqlite://data/todos.db", "data/todos.db"),
        ("sqlite:///tmp/todos.db", "/tmp/todos.db"),
        ("sqlite://MyDatabase.db", "MyDatabase.db"),
    ] {
        assert_eq!(
            ConnectionUrl::parse(url).unwrap().file_path().unwrap(),
            PathBuf::from(expected)
        );
    }
}

#[test]
fn decodes_file_path_without_form_or_fragment_rules() {
    for (url, expected) in [
        ("sqlite:/tmp/my db.sqlite", "/tmp/my db.sqlite"),
        ("sqlite:/tmp/my%20db.sqlite", "/tmp/my db.sqlite"),
        ("sqlite:/tmp/d%C3%A9j%C3%A0.db", "/tmp/déjà.db"),
        ("sqlite:/tmp/a+b.db", "/tmp/a+b.db"),
        ("sqlite:a%3Fb.db", "a?b.db"),
        ("sqlite:notes#1.db", "notes#1.db"),
    ] {
        assert_eq!(
            ConnectionUrl::parse(url).unwrap().file_path().unwrap(),
            PathBuf::from(expected)
        );
    }
}

#[test]
fn excludes_query_from_file_path() {
    for (url, expected) in [
        ("sqlite:app.db?cache=shared", "app.db"),
        ("sqlite://app.db?mode=ro", "app.db"),
        ("sqlite::memory:?cache=shared", ":memory:"),
    ] {
        assert_eq!(
            ConnectionUrl::parse(url).unwrap().file_path().unwrap(),
            PathBuf::from(expected)
        );
    }

    assert!(
        ConnectionUrl::parse("sqlite:")
            .unwrap()
            .file_path()
            .is_err()
    );
    assert!(
        ConnectionUrl::parse("sqlite://")
            .unwrap()
            .file_path()
            .is_err()
    );
}

#[test]
fn parses_authority_url_components() {
    let url = ConnectionUrl::parse("postgresql://alice:s3%3Acret@[::1]:5432/my%20db").unwrap();

    assert_eq!(url.username().unwrap().as_deref(), Some("alice"));
    assert_eq!(url.password().as_deref(), Some(&b"s3:cret"[..]));
    assert_eq!(url.host().unwrap(), Some("::1"));
    assert_eq!(url.port().unwrap(), Some(5432));
    assert_eq!(url.path(), "/my%20db");
    assert_eq!(url.decoded_path().unwrap(), "/my db");
}

#[test]
fn rejects_invalid_authority() {
    let url = ConnectionUrl::parse("dynamodb://:memory:").unwrap();
    assert!(url.validate_authority().is_err());
}

#[test]
fn decodes_query_pairs() {
    let url =
        ConnectionUrl::parse("postgresql:///mydb?host=%2Fvar%2Frun&application_name=my+app&flag")
            .unwrap();
    let pairs = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();

    assert_eq!(
        pairs,
        [
            ("host".to_string(), "/var/run".to_string()),
            ("application_name".to_string(), "my app".to_string()),
            ("flag".to_string(), String::new()),
        ]
    );
}

#[test]
fn redacts_authority_password() {
    let url = ConnectionUrl::parse("postgresql://alice:secret@localhost/mydb").unwrap();
    assert_eq!(
        url.redact_password(),
        "postgresql://alice:***@localhost/mydb"
    );

    let url = ConnectionUrl::parse("sqlite::memory:").unwrap();
    assert_eq!(url.redact_password(), "sqlite::memory:");
}
