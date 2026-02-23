mod expand;
mod schema;

use proc_macro2::TokenStream;

pub fn generate_model(input: TokenStream) -> syn::Result<TokenStream> {
    let item: syn::ItemStruct = syn::parse2(input)?;
    let model = schema::Model::from_ast(&item, false)?;

    Ok(expand::root_model(&model))
}

pub fn generate_embed(input: TokenStream) -> syn::Result<TokenStream> {
    if let Ok(item) = syn::parse2::<syn::ItemStruct>(input.clone()) {
        let model = schema::Model::from_ast(&item, true)?;
        return Ok(expand::embedded_model(&model));
    }

    let item: syn::ItemEnum = syn::parse2(input)?;
    let model = schema::Model::from_enum_ast(&item)?;
    Ok(expand::embedded_enum(&model))
}
