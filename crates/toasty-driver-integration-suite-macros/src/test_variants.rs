use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Expr, Ident, Token,
};

/// Input to the `generate_driver_test_variants!` macro
#[derive(Debug)]
struct Input {
    krate: syn::Path,
    /// The full test path (module::test_name)
    test_path: syn::Path,
    /// The driver setup expression
    driver_expr: Expr,
    /// Capabilities required by this test
    requires: Vec<String>,
    /// Capabilities supported by the driver
    capabilities: std::collections::HashMap<String, bool>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let krate: syn::Path = input.parse()?;
        input.parse::<Token![,]>()?;

        // Parse test path (module::test_name)
        let test_path: syn::Path = input.parse()?;
        input.parse::<Token![,]>()?;

        // Parse driver expression
        let driver_expr: Expr = input.parse()?;

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }

        let mut requires = Vec::new();
        let mut capabilities = std::collections::HashMap::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match &key.to_string()[..] {
                "requires" => {
                    // Parse requires: [cap1, cap2, ...]
                    let content;
                    syn::bracketed!(content in input);
                    let caps = syn::punctuated::Punctuated::<Ident, Token![,]>::parse_terminated(
                        &content,
                    )?;
                    requires.extend(caps.into_iter().map(|i| i.to_string()));
                }
                _ => {
                    // Parse capability flags: name: true/false
                    let lit: syn::LitBool = input.parse()?;
                    capabilities.insert(key.to_string(), lit.value);
                }
            }

            // Parse trailing comma if present
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Input {
            krate,
            test_path,
            driver_expr,
            requires,
            capabilities,
        })
    }
}

pub fn expand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Input);

    // Generate test variants based on what's supported
    let variants = generate_test_variants(&input);

    quote! {
        #(#variants)*
    }
    .into()
}

/// Generate test variants (id_u64, id_uuid) based on requirements
fn generate_test_variants(input: &Input) -> Vec<TokenStream2> {
    // Check if test requires capabilities that the driver doesn't have
    for req in &input.requires {
        if let Some(&supported) = input.capabilities.get(req) {
            if !supported {
                // Driver doesn't support required capability - skip this test
                return vec![];
            }
        }
        // If capability not specified, assume it's supported (default: true)
    }

    let mut variants = Vec::new();

    // Generate id_u64 variant only if auto_increment is supported (u64 IDs require auto-increment)
    let auto_increment_supported = input
        .capabilities
        .get("auto_increment")
        .copied()
        .unwrap_or(true);

    if auto_increment_supported {
        variants.push(generate_variant(input, "id_u64"));
    }

    // Always generate id_uuid variant (UUIDs don't require auto-increment)
    variants.push(generate_variant(input, "id_uuid"));

    variants
}

/// Generate a single test variant
fn generate_variant(input: &Input, variant_name: &str) -> TokenStream2 {
    let krate = &input.krate;
    let test_path = &input.test_path;
    let driver_expr = &input.driver_expr;
    let variant_ident = Ident::new(variant_name, proc_macro2::Span::call_site());

    quote! {
        #[test]
        fn #variant_ident() {
            let mut test = #krate::Test::new(
                ::std::sync::Arc::new(#driver_expr)
            );

            test.run(async move |t| {
                #krate::tests::#test_path::#variant_ident(t).await;
            });
        }
    }
}
