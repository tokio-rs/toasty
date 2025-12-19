use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, visit_mut::VisitMut, ItemFn, Type, TypePath};

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let mod_name = &input.sig.ident;
    let vis = &input.vis;

    // Generate id_u64 variant
    let id_u64_fn = generate_variant(&input, "id_u64", syn::parse_quote!(u64));

    // Generate registration for id_u64 variant
    let id_u64_registration = generate_registration(&mod_name, "id_u64");

    quote! {
        #vis mod #mod_name {
            use super::*;

            #id_u64_fn

            #id_u64_registration
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

    variant
}

/// Generate test registration code for a variant
fn generate_registration(mod_name: &syn::Ident, variant_name: &str) -> proc_macro2::TokenStream {
    let variant_ident = syn::Ident::new(variant_name, mod_name.span());
    let registration_name = syn::Ident::new(
        &format!("__{}__{}_REGISTRATION", mod_name, variant_name).to_uppercase(),
        mod_name.span(),
    );

    // Build the full test path string at compile time
    // We'll store the full path and strip the prefix at runtime in the registry lookup

    quote::quote! {
        #[::linkme::distributed_slice(crate::registry::TESTS)]
        static #registration_name: crate::registry::RegisteredTest =
            crate::registry::RegisteredTest {
                name: concat!(module_path!(), "::", stringify!(#variant_ident)),
                func: |test| ::std::boxed::Box::pin(#variant_ident(test)),
            };
    }
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
