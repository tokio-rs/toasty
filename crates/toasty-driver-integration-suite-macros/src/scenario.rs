use proc_macro::TokenStream;
use quote::quote;
use syn::visit_mut::VisitMut;
use syn::{Item, Visibility};

use crate::id_rewriter::IdRewriter;
use crate::parse::KindVariant;

pub fn expand(input: TokenStream) -> TokenStream {
    let file = syn::parse_macro_input!(input as syn::File);

    // Extract optional #![id(IDENT)] from inner attributes
    let id_ident = extract_id_ident(&file.attrs);
    let items = file.items;

    if let Some(ref id_ident) = id_ident {
        // Generate per-variant modules
        let variants = [KindVariant::IdU64, KindVariant::IdUuid];
        let modules: Vec<_> = variants
            .iter()
            .map(|variant| {
                let target_type: syn::Type = match variant {
                    KindVariant::IdU64 => syn::parse_quote!(u64),
                    KindVariant::IdUuid => syn::parse_quote!(uuid::Uuid),
                };

                let mut items = items.clone();

                // Rewrite visibility to pub(crate) and fields to pub(crate)
                for item in &mut items {
                    make_pub_crate(item);
                }

                // Rewrite ID types
                let mut rewriter = IdRewriter::new(id_ident, target_type);
                for item in &mut items {
                    rewriter.visit_item_mut(item);
                }

                let mod_name = syn::Ident::new(variant.name(), proc_macro2::Span::call_site());

                quote! {
                    pub(crate) mod #mod_name {
                        use super::*;
                        #(#items)*
                    }
                }
            })
            .collect();

        quote! { #(#modules)* }.into()
    } else {
        // No ID expansion — emit items directly with pub(crate) visibility
        let mut items = items;
        for item in &mut items {
            make_pub_crate(item);
        }

        quote! { #(#items)* }.into()
    }
}

/// Extract the ID identifier from `#![id(IDENT)]` inner attributes
fn extract_id_ident(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("id") {
            let ident: syn::Ident = attr.parse_args().expect("expected #![id(IDENT)]");
            return Some(ident.to_string());
        }
    }
    None
}

/// Set item and struct field visibility to `pub(crate)`
fn make_pub_crate(item: &mut Item) {
    let pub_crate: Visibility = syn::parse_quote!(pub(crate));

    match item {
        Item::Struct(s) => {
            s.vis = pub_crate.clone();
            // Also make fields pub(crate) so tests can access them
            for field in &mut s.fields {
                field.vis = pub_crate.clone();
            }
        }
        Item::Fn(f) => {
            f.vis = pub_crate;
        }
        Item::Enum(e) => {
            e.vis = pub_crate;
        }
        Item::Impl(_) => {
            // impl blocks don't have visibility
        }
        _ => {}
    }
}
