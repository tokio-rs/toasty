use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Token};

use crate::parse::{DriverTestAttr, ExpansionList, ThreeValuedBool};

/// Input to the `generate_driver_test_variants!` macro
/// Expects: driver_expr, crate, module_name::test_name, #[driver_test(...)], attrs(...), capability(...)
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

    /// Extra attributes (e.g., #[should_panic], #[ignore])
    extra_attrs: Vec<syn::Attribute>,

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

        // Parse attrs[...]
        let attrs_ident: syn::Ident = input.parse()?;
        if attrs_ident != "attrs" {
            return Err(input.error("expected 'attrs'"));
        }

        let attrs_content;
        syn::bracketed!(attrs_content in input);

        // Parse attributes from the bracketed content
        let extra_attrs = syn::Attribute::parse_outer(&attrs_content)?;

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
            extra_attrs,
            capabilities,
        })
    }
}

impl Input {
    /// Convert capabilities from HashMap<Ident, bool> to HashMap<String, bool>
    fn capabilities_as_strings(&self) -> HashMap<String, bool> {
        self.capabilities
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect()
    }

    /// Get the base test name (last segment of the test path)
    fn test_name(&self) -> &syn::Ident {
        &self.test_path.segments.last().unwrap().ident
    }

    /// Create a capability lookup closure for use with Expansion::should_include
    fn capability_checker(&self) -> impl Fn(&str) -> ThreeValuedBool + '_ {
        let capabilities = self.capabilities_as_strings();
        move |ident: &str| {
            match capabilities.get(ident) {
                Some(true) => ThreeValuedBool::True,
                Some(false) => ThreeValuedBool::False,
                None => ThreeValuedBool::True, // Default to true if not specified
            }
        }
    }

    /// Generate the test function name and path for a given expansion
    fn generate_test_name_and_path(
        &self,
        expansion: &crate::parse::Expansion,
    ) -> (syn::Ident, proc_macro2::TokenStream) {
        let krate = &self.krate;
        let test_path = &self.test_path;

        if let Some(expansion_ident) = expansion.to_ident() {
            // Has expansion: the function is in a module, call module::expansion_name
            // Test function name is just the expansion name (e.g., "id_uuid")
            let fn_path = quote! { #krate::tests::#test_path::#expansion_ident };
            (expansion_ident, fn_path)
        } else {
            // No expansion: call the function directly with its original name
            let base_test_name = self.test_name().clone();
            let fn_path = quote! { #krate::tests::#test_path };
            (base_test_name, fn_path)
        }
    }
}

pub fn expand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Input);

    let driver_expr = &input.driver_expr;
    let krate = &input.krate;
    let extra_attrs = &input.extra_attrs;

    // Parse the driver_test attribute to get the expansions
    let attr = DriverTestAttr::from_attribute(&input.driver_test_attr)
        .expect("Failed to parse driver_test attribute");

    // Generate expansions
    let expansions = crate::parse::DriverTest::generate_expansions(&attr);

    // Create capability checker closure
    let capability_checker = input.capability_checker();

    // Generate test functions for each expansion that passes the predicate
    let test_functions: Vec<_> = expansions
        .iter()
        .filter_map(|expansion| {
            // Evaluate the predicate for this expansion using should_include
            if !expansion.should_include(&capability_checker) {
                return None; // Skip this expansion
            }

            // Generate the test function name and path
            let (test_fn_name, fn_path) = input.generate_test_name_and_path(expansion);

            // Generate the test function with extra attributes
            Some(quote! {
                #(#extra_attrs)*
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

    // Check if we need to wrap test functions in a module
    if expansions.needs_module_wrapper() {
        // Wrap test functions in a module named after the test
        let test_module_name = input.test_name();
        quote! {
            mod #test_module_name {
                use super::*;
                #(#test_functions)*
            }
        }
        .into()
    } else {
        // Single test with no expansion - no module wrapper needed
        quote! {
            #(#test_functions)*
        }
        .into()
    }
}
