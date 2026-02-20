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
        // #[column(type = <type>)]
        // #[column("name", type = <type>)]
        // #[column(type = <type>, "name")]
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

    syn::custom_keyword!(int);
    syn::custom_keyword!(i8);
    syn::custom_keyword!(i16);
    syn::custom_keyword!(i32);
    syn::custom_keyword!(i64);

    syn::custom_keyword!(uint);
    syn::custom_keyword!(u8);
    syn::custom_keyword!(u16);
    syn::custom_keyword!(u32);
    syn::custom_keyword!(u64);

    syn::custom_keyword!(text);
    syn::custom_keyword!(varchar);

    syn::custom_keyword!(numeric);

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
    Numeric(Option<(u32, u32)>),
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
            return Ok(Self::Custom(input.parse()?));
        }

        macro_rules! peek_ident {
            ($kw:ident, $($ty:tt)*) => {
                if lookahead.peek(kw::$kw) {
                    let _kw: kw::$kw = input.parse()?;
                    return Ok(Self::$($ty)*);
                }
            };
        }
        macro_rules! peek_ident_paren_int {
            ($kw:ident, $ty:ident) => {
                if lookahead.peek(kw::$kw) {
                    let _kw: kw::$kw = input.parse()?;
                    let content;
                    parenthesized!(content in input);
                    let lit: syn::LitInt = content.parse()?;
                    return Ok(Self::$ty(lit.base10_parse()?));
                }
            };
        }

        peek_ident!(boolean, Boolean);

        peek_ident_paren_int!(int, Integer);
        peek_ident!(i8, Integer(1));
        peek_ident!(i16, Integer(2));
        peek_ident!(i32, Integer(4));
        peek_ident!(i64, Integer(8));

        peek_ident_paren_int!(uint, UnsignedInteger);
        peek_ident!(u8, UnsignedInteger(1));
        peek_ident!(u16, UnsignedInteger(2));
        peek_ident!(u32, UnsignedInteger(4));
        peek_ident!(u64, UnsignedInteger(8));

        peek_ident!(text, Text);
        peek_ident_paren_int!(varchar, VarChar);

        // numeric or numeric(precision, scale)
        if lookahead.peek(kw::numeric) {
            let _kw: kw::numeric = input.parse()?;
            if input.peek(syn::token::Paren) {
                let content;
                parenthesized!(content in input);
                let precision: syn::LitInt = content.parse()?;
                let _comma: syn::Token![,] = content.parse()?;
                let scale: syn::LitInt = content.parse()?;
                return Ok(Self::Numeric(Some((
                    precision.base10_parse()?,
                    scale.base10_parse()?,
                ))));
            } else {
                return Ok(Self::Numeric(None));
            }
        }

        peek_ident_paren_int!(binary, Binary);
        peek_ident!(blob, Blob);

        peek_ident_paren_int!(timestamp, Timestamp);
        peek_ident!(date, Date);
        peek_ident_paren_int!(time, Time);
        peek_ident_paren_int!(datetime, DateTime);

        Err(lookahead.error())
    }
}

impl ColumnType {
    /// Expand to a fully qualified `#toasty::schema::db::Type::...` token stream.
    pub(crate) fn expand_with(
        &self,
        toasty: &proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        match self {
            Self::Boolean => quote! { #toasty::schema::db::Type::Boolean },
            Self::Integer(size) => quote! { #toasty::schema::db::Type::Integer(#size) },
            Self::UnsignedInteger(size) => {
                quote! { #toasty::schema::db::Type::UnsignedInteger(#size) }
            }
            Self::Text => quote! { #toasty::schema::db::Type::Text },
            Self::VarChar(size) => quote! { #toasty::schema::db::Type::VarChar(#size) },
            Self::Numeric(None) => quote! { #toasty::schema::db::Type::Numeric(None) },
            Self::Numeric(Some((precision, scale))) => {
                quote! { #toasty::schema::db::Type::Numeric(Some((#precision, #scale))) }
            }
            Self::Binary(size) => quote! { #toasty::schema::db::Type::Binary(#size) },
            Self::Blob => quote! { #toasty::schema::db::Type::Blob },
            Self::Timestamp(precision) => {
                quote! { #toasty::schema::db::Type::Timestamp(#precision) }
            }
            Self::Date => quote! { #toasty::schema::db::Type::Date },
            Self::Time(precision) => quote! { #toasty::schema::db::Type::Time(#precision) },
            Self::DateTime(precision) => quote! { #toasty::schema::db::Type::DateTime(#precision) },
            Self::Custom(custom) => quote! { #toasty::schema::db::Type::Custom(#custom) },
        }
    }
}
