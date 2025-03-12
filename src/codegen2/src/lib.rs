mod expand;
mod schema;

use proc_macro2::TokenStream;
use quote::quote;

pub fn generate(args: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let mut item: syn::ItemStruct = syn::parse2(input)?;
    let model = schema::Model::from_ast(&mut item, args)?;
    let gen = expand::model(&model);

    Ok(quote! {
        #item
        #gen
    })
}
