mod expand;
mod schema;

use proc_macro2::TokenStream;

pub fn generate_model(input: TokenStream) -> syn::Result<TokenStream> {
    let item: syn::ItemStruct = syn::parse2(input)?;
    let model = schema::Model::from_ast(&item)?;

    Ok(expand::model(&model))
}
