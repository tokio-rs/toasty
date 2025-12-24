use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    spanned::Spanned,
    Expr, Ident, Token,
};

/// Input to the `generate_driver_test_variants!` macro
#[derive(Debug)]
struct Input {
    krate: syn::Path,
    /// The full test path (module::test_name)
    test_path: syn::Path,
    /// The driver setup expression
    driver_expr: Expr,

    auto_increment: bool,
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

        let mut auto_increment = true;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            // Parse regular option
            let lit: syn::LitBool = input.parse()?;

            // Parse trailing comma if present
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }

            match &key.to_string()[..] {
                "auto_increment" => auto_increment = lit.value,
                key => Err(syn::Error::new(key.span(), "invalid option"))?,
            }
        }

        Ok(Input {
            krate,
            test_path,
            driver_expr,
            auto_increment,
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
    let mut variants = Vec::new();

    // Generate id_u64 variant (only if auto_increment is available or not required)
    if input.auto_increment {
        variants.push(generate_variant(input, "id_u64"));
    }

    // Generate id_uuid variant (always, unless specifically excluded in the future)
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
