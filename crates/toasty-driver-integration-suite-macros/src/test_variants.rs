use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Token};

/// Input to the `generate_driver_test_variants!` macro
/// Expects: module_name::test_name, #[driver_test(...)]
#[derive(Debug)]
struct Input {
    /// The full test path (module::test_name)
    test_path: syn::Path,
    /// The driver_test attribute
    driver_test_attr: syn::Attribute,
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

        // Consume any trailing tokens
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if !input.is_empty() {
                let _: proc_macro2::TokenTree = input.parse()?;
            }
        }

        Ok(Input {
            test_path,
            driver_test_attr,
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

    // Debug print the attribute
    let attr_debug = format!("{:#?}", input.driver_test_attr);

    // Generate a single stub test
    quote! {
        #[test]
        fn #test_ident() {
            println!("{}", #attr_debug);
        }
    }
    .into()
}
