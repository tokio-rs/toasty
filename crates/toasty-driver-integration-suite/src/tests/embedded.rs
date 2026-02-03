use crate::prelude::*;

#[driver_test]
pub async fn basic_embedded_struct(test: &mut Test) {
    #[derive(toasty::Embed)]
    struct Address {
        street: String,
        city: String,
    }

    // Register the embedded model with the database schema
    let mut builder = toasty::Db::builder();
    builder.register::<Address>();

    // For now, just verify the struct with #[derive(Embed)] compiles
    // and can be registered.
    let _ = Address {
        street: "123 Main St".to_string(),
        city: "Springfield".to_string(),
    };
}
