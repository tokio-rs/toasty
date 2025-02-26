use proc_macro2::TokenStream;

pub(crate) fn int(v: usize) -> TokenStream {
    use std::str::FromStr;
    TokenStream::from_str(&v.to_string()).expect("failed to parse int")
}
