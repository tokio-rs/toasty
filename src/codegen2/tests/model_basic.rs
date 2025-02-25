use quote::quote;
use toasty_codegen2::generate;

#[test]
fn test_model_basic() {
    let out = generate(quote! {
        pub struct User {
            id: i32,
            name: String,
        }
    });

    panic!("{out:#?}");
}
