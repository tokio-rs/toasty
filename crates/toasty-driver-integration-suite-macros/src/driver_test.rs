use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, visit_mut::VisitMut, ItemFn, Type, TypePath};

use crate::parse::DriverTest;

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    // Parse the driver test
    let driver_test = DriverTest::from_item_fn(input);

    let mod_name = &driver_test.name;
    let vis = &driver_test.input.vis;

    // Generate variants
    let variant_fns: Vec<_> = driver_test
        .kinds
        .iter()
        .map(|kind| generate_variant(&driver_test.input, kind.name(), kind.target_type()))
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
fn generate_variant(input: &ItemFn, variant_name: &str, target_type: Type) -> ItemFn {
    let mut variant = input.clone();

    // Update function name
    variant.sig.ident = syn::Ident::new(variant_name, input.sig.ident.span());

    // Rewrite ID types to target type
    let mut rewriter = IdRewriter::new(target_type);
    rewriter.visit_item_fn_mut(&mut variant);

    // For id_u64 variant, inject auto_increment capability check at the start
    if variant_name == "id_u64" {
        // Get the parameter name from the function signature (should be first parameter)
        let param_name = if let Some(syn::FnArg::Typed(pat_type)) = variant.sig.inputs.first() {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                &pat_ident.ident
            } else {
                panic!("Expected identifier pattern for first parameter");
            }
        } else {
            panic!("Expected at least one parameter");
        };

        let original_block = &variant.block;
        variant.block = syn::parse_quote! {{
            // Skip test if auto_increment not supported (for id_u64 variant)
            if !#param_name.capability().has_auto_increment {
                return;
            }
            #original_block
        }};
    }

    variant
}

/// Visitor that rewrites `ID` type references to a configurable target type
struct IdRewriter {
    target_type: Type,
}

impl IdRewriter {
    fn new(target_type: Type) -> Self {
        Self { target_type }
    }
}

impl VisitMut for IdRewriter {
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(TypePath { qself: None, path }) = ty {
            // Check if this is a simple `ID` identifier
            if path.segments.len() == 1 && path.segments[0].ident == "ID" {
                // Replace with target type
                *ty = self.target_type.clone();
                return;
            }
        }

        // Continue visiting nested types
        syn::visit_mut::visit_type_mut(self, ty);
    }
}
