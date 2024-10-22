use proc_macro2::{Span, TokenStream};

pub(crate) fn ident(name: &str) -> syn::Ident {
    syn::Ident::new(name, Span::call_site())
}

macro_rules! ident {
    ( $($t:tt)* ) => {
        $crate::util::ident(&format!( $($t)* ))
    }
}

pub(crate) fn int(v: usize) -> TokenStream {
    use std::str::FromStr;
    TokenStream::from_str(&v.to_string()).expect("failed to parse int")
}

pub(crate) fn type_name(name: &str) -> String {
    std_util::str::upper_camel_case(name)
}

pub(crate) fn const_name(name: &str) -> String {
    std_util::str::upper_snake_case(name)
}
