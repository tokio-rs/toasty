use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, token::Token, Token};

/// Input to the `generate_driver_test_variants!` macro
/// Expects: module_name::test_name, #[driver_test(...)], capability(...)
#[derive(Debug)]
struct Input {
    /// The full test path (module::test_name)
    test_path: syn::Path,
    /// The driver_test attribute
    driver_test_attr: syn::Attribute,
    /// The capability arguments as a string
    capabilities: HashMap<syn::Ident, bool>,
}

impl syn::parse::Parse for Input {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let test_path: syn::Path = input.parse()?;
        input.parse::<Token![,]>()?;

        // Parse a single attribute using Attribute::parse_outer
        let attrs = syn::Attribute::parse_outer(input)?;
        let driver_test_attr = attrs
            .into_iter()
            .next()
            .ok_or_else(|| input.error("expected driver_test attribute"))?;

        input.parse::<Token![,]>()?;

        // Parse capability(...)
        let capability_ident: syn::Ident = input.parse()?;
        if capability_ident != "capability" {
            return Err(input.error("expected 'capability'"));
        }

        let content;
        syn::parenthesized!(content in input);

        let mut capabilities = HashMap::new();
        while !content.is_empty() {
            let key: syn::Ident = content.parse()?;
            content.parse::<Token![:]>()?;
            let lit: syn::LitBool = content.parse()?;

            capabilities.insert(key, lit.value);

            // Parse trailing comma if present
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(Input {
            test_path,
            driver_test_attr,
            capabilities,
        })
    }
}

pub fn expand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Input);

    // Extract the test name (last segment of the path)
    let test_name = input
        .test_path
        .segments
        .last()
        .expect("test path should have at least one segment");
    let test_ident = &test_name.ident;

    // Debug print the attribute and capability args
    let attr_debug = format!("{:#?}", input.driver_test_attr);
    let capabilities = format!("{:#?}", input.capabilities);

    // Generate a single stub test
    quote! {
        #[test]
        fn #test_ident() {
            println!("Attribute:\n{}", #attr_debug);
            println!("\nCapabilities: {}", #capabilities);
        }
    }
    .into()
}
