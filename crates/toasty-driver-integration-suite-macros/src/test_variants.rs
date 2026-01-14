use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Token};

use crate::parse::{BoolExpr, DriverTestAttr, Expansion};

/// Input to the `generate_driver_test_variants!` macro
/// Expects: module_name::test_name, #[driver_test(...)], capability(...)
#[derive(Debug)]
struct Input {
    /// Crate
    krate: syn::Path,

    /// The full test path (module::test_name)
    test_path: syn::Path,

    // The driver expression
    driver_expr: syn::Expr,

    /// The driver_test attribute
    driver_test_attr: syn::Attribute,
    /// Parsed capability flags
    capabilities: HashMap<syn::Ident, bool>,
}

impl syn::parse::Parse for Input {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let driver_expr: syn::Expr = input.parse()?;
        input.parse::<Token![,]>()?;

        let krate: syn::Path = input.parse()?;
        input.parse::<Token![,]>()?;

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
            driver_expr,
            krate,
            test_path,
            driver_test_attr,
            capabilities,
        })
    }
}

pub fn expand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Input);

    let driver_expr = &input.driver_expr;
    let krate = &input.krate;

    // Parse the driver_test attribute to get the expansions
    let attr = DriverTestAttr::from_attribute(&input.driver_test_attr)
        .expect("Failed to parse driver_test attribute");

    // Convert capabilities HashMap<Ident, bool> to HashMap<String, bool>
    let capabilities: HashMap<String, bool> = input
        .capabilities
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();

    // Extract the test path
    let test_path = &input.test_path;

    // Generate expansions
    let expansions = crate::parse::DriverTest::generate_expansions(&attr);

    // Generate test functions for each expansion that passes the predicate
    let test_functions: Vec<_> = expansions
        .iter()
        .filter_map(|expansion| {
            // Evaluate the predicate for this expansion
            if !evaluate_predicate(&attr.requires, expansion, &capabilities) {
                return None; // Skip this expansion
            }

            // Generate the test function name
            let expansion_name = expansion.name();
            let test_fn_name = if expansion_name.is_empty() {
                // No expansion suffix
                test_path.segments.last().unwrap().ident.clone()
            } else {
                // Combine test name with expansion suffix
                syn::Ident::new(
                    &format!(
                        "{}_{}",
                        test_path.segments.last().unwrap().ident,
                        expansion_name
                    ),
                    proc_macro2::Span::call_site(),
                )
            };

            // Determine the actual function path to call
            let fn_path = if expansion_name.is_empty() {
                // Call the function directly
                quote! { #krate::tests::#test_path }
            } else {
                // Call the expansion function within the module
                let expansion_ident =
                    syn::Ident::new(&expansion_name, proc_macro2::Span::call_site());
                quote! { #krate::tests::#test_path::#expansion_ident }
            };

            // Generate the test function
            Some(quote! {
                #[test]
                fn #test_fn_name() {
                    let mut test = #krate::Test::new(
                        ::std::sync::Arc::new(#driver_expr)
                    );

                    test.run(async move |t| {
                        #fn_path(t).await;
                    });
                }
            })
        })
        .collect();

    let ident = test_path.segments.last().unwrap().ident.clone();
    let output = format!("{:#?}", test_functions);

    quote! {
        #(#test_functions)*
    }
    .into()
}

/// Evaluate the predicate for a given expansion and capabilities
fn evaluate_predicate(
    requires: &Option<BoolExpr>,
    expansion: &Expansion,
    capabilities: &HashMap<String, bool>,
) -> bool {
    // If no requires clause, the test always passes
    let Some(ref expr) = requires else {
        return true;
    };

    // Evaluate the boolean expression
    evaluate_bool_expr(expr, expansion, capabilities)
}

/// Recursively evaluate a boolean expression
fn evaluate_bool_expr(
    expr: &BoolExpr,
    expansion: &Expansion,
    capabilities: &HashMap<String, bool>,
) -> bool {
    match expr {
        BoolExpr::Ident(name) => {
            // Check if this is a matrix dimension or expansion identifier (e.g., "single", "id_u64")
            if expansion.is_ident_true(name) {
                return true;
            }
            // Otherwise check capabilities (defaulting to true if not specified)
            capabilities.get(name).copied().unwrap_or(true)
        }
        BoolExpr::Or(exprs) => exprs
            .iter()
            .any(|e| evaluate_bool_expr(e, expansion, capabilities)),
        BoolExpr::And(exprs) => exprs
            .iter()
            .all(|e| evaluate_bool_expr(e, expansion, capabilities)),
        BoolExpr::Not(inner) => !evaluate_bool_expr(inner, expansion, capabilities),
    }
}
