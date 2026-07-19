use toasty_core::{schema::db, stmt};

#[test]
fn list_bridge_type_recurses_into_elements() {
    let storage = db::Type::list(db::Type::UnsignedInteger(1));
    let app = stmt::Type::List(Box::new(stmt::Type::I64));

    assert_eq!(
        storage.bridge_type(&app),
        stmt::Type::List(Box::new(stmt::Type::U8))
    );
}
