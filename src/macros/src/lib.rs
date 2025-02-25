extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;

#[proc_macro_attribute]
pub fn model(_args: TokenStream, input: TokenStream) -> TokenStream {
    match toasty_codegen2::generate(input.into()) {
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

#[proc_macro]
pub fn schema(input: TokenStream) -> TokenStream {
    let schema_src = match syn::parse::<syn::LitStr>(input) {
        Ok(v) => v.value(),
        Err(e) => return e.to_compile_error().into(),
    };

    let schema = match toasty_core::schema::from_str(&schema_src) {
        Ok(schema) => schema,
        Err(e) => {
            return syn::Error::new(Span::call_site(), e.to_string())
                .to_compile_error()
                .into();
        }
    };

    let codegen_output = toasty_codegen::generate(&schema, true);

    let mods = codegen_output.models.iter().map(|model_output| {
        let struct_name = ident(&model_output.model.name.upper_camel_case());
        let module_name = &model_output.module_name;
        let body = &model_output.body;

        quote! {
            pub mod #module_name {
                #body
            }

            pub use #module_name::#struct_name;
        }
    });

    quote! {
        pub mod db {
            #( #mods )*

            pub fn load_schema() -> toasty::schema::app::Schema {
                toasty::schema::from_str(#schema_src).unwrap()
            }
        }
    }
    .into()
}

fn ident(name: &str) -> syn::Ident {
    syn::Ident::new(name, Span::call_site())
}
