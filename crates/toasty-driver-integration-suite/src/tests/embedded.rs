use crate::prelude::*;

#[driver_test]
pub async fn basic_embedded_struct(_test: &mut Test) {
    #[derive(toasty::Embed)]
    struct Address {
        street: String,
        city: String,
    }

    // Register the embedded model with the database schema
    let mut builder = toasty::Db::builder();
    builder.register::<Address>();

    // Verify the Address type is in the app-level schema
    let schema = <Address as toasty::Register>::schema();

    assert_struct!(schema, toasty::schema::app::Model {
        name.upper_camel_case(): "Address",
        kind: toasty::schema::app::ModelKind::Embedded,
        fields: [
            _ { name.app_name: "street", .. },
            _ { name.app_name: "city", .. }
        ],
        ..
    });
}
