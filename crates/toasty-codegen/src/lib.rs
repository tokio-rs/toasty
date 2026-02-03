mod expand;
mod schema;

use proc_macro2::TokenStream;
use quote::quote;

pub fn generate_model(input: TokenStream) -> syn::Result<TokenStream> {
    let item: syn::ItemStruct = syn::parse2(input)?;
    let model = schema::Model::from_ast(&item)?;

    Ok(expand::model(&model))
}

pub fn generate_embed(input: TokenStream) -> syn::Result<TokenStream> {
    let input: syn::DeriveInput = syn::parse2(input)?;
    let name = &input.ident;

    let expanded = quote! {
        const _: () = {
            use toasty as _toasty;

            impl _toasty::Register for #name {
                fn id() -> _toasty::codegen_support::ModelId {
                    static ID: std::sync::OnceLock<_toasty::codegen_support::ModelId> = std::sync::OnceLock::new();
                    *ID.get_or_init(|| _toasty::codegen_support::generate_unique_id())
                }

                fn schema() -> _toasty::codegen_support::schema::app::Model {
                    use _toasty::codegen_support::schema;

                    // For now, return a minimal embedded model with no fields
                    // This is a no-op registration
                    schema::app::Model {
                        id: Self::id(),
                        name: schema::Name::new(stringify!(#name)),
                        fields: vec![],
                        kind: schema::app::ModelKind::Embedded,
                        indices: vec![],
                        table_name: None,
                    }
                }
            }

            impl _toasty::Embed for #name {}
        };
    };

    Ok(expanded)
}
