#[derive(Debug)]
pub enum AutoStrategy {
    Unspecified,
    Uuid(UuidVersion),
    Increment,
}

#[derive(Debug)]
pub enum UuidVersion {
    V4,
    V7,
}

impl AutoStrategy {
    pub(super) fn from_ast(attr: &syn::Attribute) -> syn::Result<Self> {
        match attr.meta {
            syn::Meta::Path(_) => Ok(Self::Unspecified),
            _ => attr.parse_args(),
        }
    }
}

mod kw {
    syn::custom_keyword!(uuid);
    syn::custom_keyword!(v4);
    syn::custom_keyword!(v7);

    syn::custom_keyword!(increment);
}

impl syn::parse::Parse for AutoStrategy {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::uuid) {
            let _: kw::uuid = input.parse()?;
            let content;
            let _ = syn::parenthesized!(content in input);
            Ok(Self::Uuid(content.parse()?))
        } else if lookahead.peek(kw::increment) {
            let _: kw::increment = input.parse()?;
            Ok(Self::Increment)
        } else {
            Err(lookahead.error())
        }
    }
}

impl syn::parse::Parse for UuidVersion {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::v4) {
            let _: kw::v4 = input.parse()?;
            Ok(Self::V4)
        } else if lookahead.peek(kw::v7) {
            let _: kw::v7 = input.parse()?;
            Ok(Self::V7)
        } else {
            Err(lookahead.error())
        }
    }
}
