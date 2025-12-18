extern crate proc_macro;

mod driver_test;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn driver_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    driver_test::expand(attr, item)
}
