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
