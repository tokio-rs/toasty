use crate::prelude::*;

#[driver_test]
pub async fn basic_embedded_struct(test: &mut Test) {
    #[derive(toasty::Embed)]
    struct Address {
        street: String,
        city: String,
    }

    // For now, just verify the struct with #[derive(Embed)] compiles.
    let _ = Address {
        street: "123 Main St".to_string(),
        city: "Springfield".to_string(),
    };
}
