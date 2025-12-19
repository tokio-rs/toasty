extern crate proc_macro;

mod driver_test;
mod parse;
mod test_registry;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn driver_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    driver_test::expand(attr, item)
}

#[proc_macro]
pub fn generate_test_registry(input: TokenStream) -> TokenStream {
    test_registry::expand(input)
}
