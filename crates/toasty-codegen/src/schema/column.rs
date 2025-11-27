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
    syn::custom_keyword!(boolean);
    syn::custom_keyword!(integer);
    syn::custom_keyword!(unsignedinteger);
    syn::custom_keyword!(text);
    syn::custom_keyword!(varchar);
    syn::custom_keyword!(binary);
    syn::custom_keyword!(blob);
    syn::custom_keyword!(timestamp);
    syn::custom_keyword!(date);
    syn::custom_keyword!(time);
    syn::custom_keyword!(datetime);
}

#[derive(Debug)]
pub enum ColumnType {
    Boolean,
    Integer(u8),
    UnsignedInteger(u8),
    Text,
    VarChar(u64),
    Binary(u64),
    Blob,
    Timestamp(u8),
    Date,
    Time(u8),
    DateTime(u8),
    Custom(syn::LitStr),
}

impl syn::parse::Parse for ColumnType {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(syn::LitStr) {
            Ok(Self::Custom(input.parse()?))
        } else if lookahead.peek(kw::boolean) {
            let _kw: kw::boolean = input.parse()?;
            Ok(Self::Boolean)
        } else if lookahead.peek(kw::integer) {
            let _kw: kw::integer = input.parse()?;
            let content;
            parenthesized!(content in input);
            let lit: syn::LitInt = content.parse()?;
            Ok(Self::Integer(lit.base10_parse()?))
        } else if lookahead.peek(kw::unsignedinteger) {
            let _kw: kw::unsignedinteger = input.parse()?;
            let content;
            parenthesized!(content in input);
            let lit: syn::LitInt = content.parse()?;
            Ok(Self::UnsignedInteger(lit.base10_parse()?))
        } else if lookahead.peek(kw::text) {
            let _kw: kw::text = input.parse()?;
            Ok(Self::Text)
        } else if lookahead.peek(kw::varchar) {
            let _kw: kw::varchar = input.parse()?;
            let content;
            parenthesized!(content in input);
            let lit: syn::LitInt = content.parse()?;
            Ok(Self::VarChar(lit.base10_parse()?))
        } else if lookahead.peek(kw::binary) {
            let _kw: kw::binary = input.parse()?;
            let content;
            parenthesized!(content in input);
            let lit: syn::LitInt = content.parse()?;
            Ok(Self::Binary(lit.base10_parse()?))
        } else if lookahead.peek(kw::blob) {
            let _kw: kw::blob = input.parse()?;
            Ok(Self::Blob)
        } else if lookahead.peek(kw::timestamp) {
            let _kw: kw::timestamp = input.parse()?;
            let content;
            parenthesized!(content in input);
            let lit: syn::LitInt = content.parse()?;
            Ok(Self::Timestamp(lit.base10_parse()?))
        } else if lookahead.peek(kw::date) {
            let _kw: kw::date = input.parse()?;
            Ok(Self::Date)
        } else if lookahead.peek(kw::time) {
            let _kw: kw::time = input.parse()?;
            let content;
            parenthesized!(content in input);
            let lit: syn::LitInt = content.parse()?;
            Ok(Self::Time(lit.base10_parse()?))
        } else if lookahead.peek(kw::datetime) {
            let _kw: kw::datetime = input.parse()?;
            let content;
            parenthesized!(content in input);
            let lit: syn::LitInt = content.parse()?;
            Ok(Self::DateTime(lit.base10_parse()?))
        } else {
            Err(lookahead.error())
        }
    }
}

impl quote::ToTokens for ColumnType {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::Boolean => quote! { db::Type::Boolean },
            Self::Integer(size) => quote! { db::Type::Integer(#size) },
            Self::UnsignedInteger(size) => quote! { db::Type::UnsignedInteger(#size) },
            Self::Text => quote! { db::Type::Text },
            Self::VarChar(size) => quote! { db::Type::VarChar(#size) },
            Self::Binary(size) => quote! { db::Type::Binary(#size) },
            Self::Blob => quote! { db::Type::Blob },
            Self::Timestamp(precision) => quote! { db::Type::Timestamp(#precision) },
            Self::Date => quote! { db::Type::Date },
            Self::Time(precision) => quote! { db::Type::Time(#precision) },
            Self::DateTime(precision) => quote! { db::Type::DateTime(#precision) },
            Self::Custom(custom) => quote! { db::Type::Custom(#custom) },
        }
        .to_tokens(tokens);
    }
}
