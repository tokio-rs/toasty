use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, visit_mut::VisitMut, ItemFn, Type, TypePath};

use crate::parse::{DriverTest, DriverTestAttr, Kind};

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let attr = parse_macro_input!(attr as DriverTestAttr);

    // Parse the driver test using shared logic
    let driver_test = DriverTest::from_item_fn(input, attr);

    // If no kinds, just return the original function (no expansion)
    if driver_test.kinds.is_empty() {
        let input = &driver_test.input;
        return quote! {
            #input
        }
        .into();
    }

    let mod_name = &driver_test.name;
    let vis = &driver_test.input.vis;

    // Generate variants using shared Kind logic
    let variant_fns: Vec<_> = driver_test
        .kinds
        .iter()
        .map(|kind| generate_variant(&driver_test.input, kind, &driver_test.requires))
        .collect();

    quote! {
        #vis mod #mod_name {
            use super::*;

            #(#variant_fns)*
        }
    }
    .into()
}

/// Generate a test variant with ID rewritten to the target type
fn generate_variant(input: &ItemFn, kind: &Kind, requires: &[String]) -> ItemFn {
    let mut variant = input.clone();

    // Update function name using Kind's method
    variant.sig.ident = syn::Ident::new(kind.name(), input.sig.ident.span());

    // Rewrite ID types to target type using Kind's configuration
    let mut rewriter = IdRewriter::new(kind.ident(), kind.target_type());
    rewriter.visit_item_fn_mut(&mut variant);

    // Add capability checks at the beginning of the function if there are requires
    if !requires.is_empty() {
        add_capability_checks(&mut variant, requires);
    }

    variant
}

/// Add runtime capability checks to the beginning of a test function
fn add_capability_checks(func: &mut ItemFn, requires: &[String]) {
    use syn::{parse_quote, Ident, Stmt};

    // Get the test parameter name (first parameter of the function)
    let test_param = func
        .sig
        .inputs
        .first()
        .and_then(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    Some(&pat_ident.ident)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("Test function must have at least one parameter");

    // Generate capability check statements
    let capability_checks: Vec<Stmt> = requires
        .iter()
        .map(|cap| {
            let cap_ident = Ident::new(cap, proc_macro2::Span::call_site());
            parse_quote! {
                assert!(
                    #test_param.capability().#cap_ident,
                    "Driver does not support required capability: {}",
                    stringify!(#cap_ident)
                );
            }
        })
        .collect();

    // Prepend the checks to the function body
    let original_block = &func.block;
    func.block = parse_quote! {
        {
            #(#capability_checks)*
            #original_block
        }
    };
}

/// Visitor that rewrites type references to a configurable target type
struct IdRewriter {
    /// The identifier to replace (e.g., "ID")
    ident: String,
    /// The target type to replace with
    target_type: Type,
}

impl IdRewriter {
    fn new(ident: &str, target_type: Type) -> Self {
        Self {
            ident: ident.to_string(),
            target_type,
        }
    }
}

impl VisitMut for IdRewriter {
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(TypePath { qself: None, path }) = ty {
            // Check if this matches the identifier we're looking for
            if path.segments.len() == 1 && path.segments[0].ident == self.ident {
                // Replace with target type
                *ty = self.target_type.clone();
                return;
            }
        }

        // Continue visiting nested types
        syn::visit_mut::visit_type_mut(self, ty);
    }
}
