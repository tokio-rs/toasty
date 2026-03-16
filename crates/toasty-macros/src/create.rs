mod expand;
mod parse;

use proc_macro2::TokenStream;

pub(crate) fn generate(input: TokenStream) -> syn::Result<TokenStream> {
    let parsed: parse::CreateItem = syn::parse2(input)?;
    Ok(expand::expand(&parsed))
}
