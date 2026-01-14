extern crate proc_macro;

mod driver_test;
mod parse;
mod test_registry;
mod test_variants;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn driver_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    driver_test::expand(attr, item)
}

#[proc_macro]
pub fn generate_test_registry(input: TokenStream) -> TokenStream {
    test_registry::expand(input)
}

#[proc_macro]
pub fn generate_driver_test_variants(input: TokenStream) -> TokenStream {
    test_variants::expand(input)
}

/// Expression macro that evaluates to true or false based on the current driver test expansion.
/// This is rewritten by the #[driver_test] attribute macro based on the expansion context.
#[proc_macro]
pub fn driver_test_cfg(_input: TokenStream) -> TokenStream {
    // This should never be called directly - it should be rewritten by #[driver_test]
    // If it is called, emit a helpful error
    let error = syn::Error::new(
        proc_macro2::Span::call_site(),
        "driver_test_cfg! can only be used inside a #[driver_test] function",
    );
    error.to_compile_error().into()
}
