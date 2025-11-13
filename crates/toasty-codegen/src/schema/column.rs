use quote::quote;
use syn::parenthesized;

#[derive(Debug)]
pub(crate) struct Column {
    pub(crate) name: Option<syn::LitStr>,
    pub(crate) ty: Option<ColumnType>,
}

impl Column {
    pub(super) fn from_ast(attr: &syn::Attribute) -> syn::Result<Column> {
        attr.parse_args()
    }
}

impl syn::parse::Parse for Column {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut result = Self {
            name: None,
            ty: None,
        };

        // Loop through the list of comma separated arguments to fill in the result one by one.
        //
        // Allowed syntax:
        //
        // #[column("name")]
        // #[column(type = "type")]
        // #[column("name", type = "type")]
        // #[column(type = "type", "name")]
        loop {
            let lookahead = input.lookahead1();

            if lookahead.peek(syn::LitStr) {
                if result.name.is_some() {
                    return Err(syn::Error::new(input.span(), "duplicate column name"));
                }
                result.name = Some(input.parse()?);
            } else if lookahead.peek(syn::Token![type]) {
                if result.ty.is_some() {
                    return Err(syn::Error::new(input.span(), "duplicate column type"));
                }
                let _type_token: syn::Token![type] = input.parse()?;
                let _eq_token: syn::Token![=] = input.parse()?;
                result.ty = Some(input.parse()?);
            } else {
                return Err(lookahead.error());
            }

            if input.is_empty() {
                break;
            }
            let _comma_token: syn::Token![,] = input.parse()?;
        }

        Ok(result)
    }
}

mod kw {
    syn::custom_keyword!(varchar);
}

#[derive(Debug)]
pub enum ColumnType {
    VarChar(u64),
    Custom(syn::LitStr),
}

impl syn::parse::Parse for ColumnType {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(syn::LitStr) {
            Ok(Self::Custom(input.parse()?))
        } else if lookahead.peek(kw::varchar) {
            let _kw: kw::varchar = input.parse()?;
            let content;
            parenthesized!(content in input);
            let lit: syn::LitInt = content.parse()?;
            Ok(Self::VarChar(lit.base10_parse()?))
        } else {
            Err(lookahead.error())
        }
    }
}

impl quote::ToTokens for ColumnType {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::VarChar(size) => quote! { db::Type::VarChar(#size) },
            Self::Custom(custom) => quote! { db::Type::Custom(#custom) },
        }
        .to_tokens(tokens);
    }
}
