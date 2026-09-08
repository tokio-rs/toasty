#![cfg(feature = "net")]

use toasty_core::stmt::{Type, Value};

#[test]
fn network_values_cast_to_and_from_text() {
    let cases = [
        (Type::Cidr, "2001:db8::/32"),
        (Type::Inet, "2001:db8::1/64"),
        (Type::MacAddr, "ac:de:48:23:45:67"),
        (Type::MacAddr8, "ac:de:48:23:45:67:89:ab"),
    ];

    for (ty, text) in cases {
        let value = ty.cast(&(), Value::String(text.to_owned())).unwrap();
        assert_eq!(value.infer_ty(), ty);
        let encoded = Type::String.cast(&(), value.clone()).unwrap();
        assert_eq!(ty.cast(&(), encoded).unwrap(), value);
    }
}

#[test]
fn cidr_rejects_host_bits_but_inet_preserves_them() {
    let text = Value::String("192.0.2.1/24".to_owned());

    assert!(Type::Cidr.cast(&(), text.clone()).is_err());
    assert_eq!(
        Type::Inet.cast(&(), text).unwrap(),
        Value::Inet("192.0.2.1/24".parse().unwrap())
    );
}

#[test]
fn network_values_use_backend_storage_defaults() {
    use toasty_core::{driver::StorageTypes, schema::db};

    let cidr = Value::Cidr("10.0.0.0/8".parse().unwrap());
    let inet = Value::Inet("10.0.0.1/8".parse().unwrap());

    assert_eq!(
        cidr.infer_db_ty(&StorageTypes::POSTGRESQL).unwrap(),
        db::Type::Cidr
    );
    assert_eq!(
        inet.infer_db_ty(&StorageTypes::SQLITE).unwrap(),
        db::Type::Text
    );
    assert_eq!(
        inet.infer_db_ty(&StorageTypes::MYSQL).unwrap(),
        db::Type::VarChar(43)
    );
}
