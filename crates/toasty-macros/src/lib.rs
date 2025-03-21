extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn model(args: TokenStream, input: TokenStream) -> TokenStream {
    match toasty_codegen::generate(args.into(), input.into()) {
        Ok(output) => output.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn include_schema(_input: TokenStream) -> TokenStream {
    todo!()
}

#[proc_macro]
pub fn query(_input: TokenStream) -> TokenStream {
    quote!(println!("TODO")).into()
}

#[proc_macro]
pub fn create(_input: TokenStream) -> TokenStream {
    quote!(println!("TODO")).into()
}
