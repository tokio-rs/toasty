use quote::quote;
use syn::parenthesized;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VariantValue {
    Integer(i64),
    String(String),
}

impl VariantValue {
    pub(crate) fn is_integer(&self) -> bool {
        matches!(self, Self::Integer(_))
    }

    pub(crate) fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }
}

impl std::fmt::Display for VariantValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Integer(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "\"{s}\""),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Column {
    pub(crate) name: Option<syn::LitStr>,
    pub(crate) ty: Option<ColumnType>,
    pub(crate) variant: Option<VariantValue>,
    pub(crate) rename_all: Option<RenameRule>,
}

/// A case-conversion rule applied to enum variant identifiers to derive their
/// default string labels, spelled to match serde's `rename_all` vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenameRule {
    Lower,
    Upper,
    Pascal,
    Camel,
    Snake,
    ScreamingSnake,
    Kebab,
    ScreamingKebab,
}

impl RenameRule {
    fn from_lit(lit: &syn::LitStr) -> syn::Result<Self> {
        Ok(match lit.value().as_str() {
            "lowercase" => Self::Lower,
            "UPPERCASE" => Self::Upper,
            "PascalCase" => Self::Pascal,
            "camelCase" => Self::Camel,
            "snake_case" => Self::Snake,
            "SCREAMING_SNAKE_CASE" => Self::ScreamingSnake,
            "kebab-case" => Self::Kebab,
            "SCREAMING-KEBAB-CASE" => Self::ScreamingKebab,
            other => {
                return Err(syn::Error::new_spanned(
                    lit,
                    format!(
                        "unknown rename_all rule \"{other}\"; expected one of \
                         \"lowercase\", \"UPPERCASE\", \"PascalCase\", \"camelCase\", \
                         \"snake_case\", \"SCREAMING_SNAKE_CASE\", \"kebab-case\", \
                         \"SCREAMING-KEBAB-CASE\""
                    ),
                ));
            }
        })
    }

    /// Applies the rule to a variant identifier, returning the derived label.
    pub(crate) fn apply(self, ident: &str) -> String {
        use heck::{
            ToKebabCase, ToLowerCamelCase, ToShoutyKebabCase, ToShoutySnakeCase, ToSnakeCase,
            ToUpperCamelCase,
        };
        match self {
            Self::Lower => ident.to_lowercase(),
            Self::Upper => ident.to_uppercase(),
            Self::Pascal => ident.to_upper_camel_case(),
            Self::Camel => ident.to_lower_camel_case(),
            Self::Snake => ident.to_snake_case(),
            Self::ScreamingSnake => ident.to_shouty_snake_case(),
            Self::Kebab => ident.to_kebab_case(),
            Self::ScreamingKebab => ident.to_shouty_kebab_case(),
        }
    }
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
            variant: None,
            rename_all: None,
        };

        // Loop through the list of comma separated arguments to fill in the result one by one.
        //
        // Allowed syntax:
        //
        // #[column("name")]
        // #[column(type = <type>)]
        // #[column("name", type = <type>)]
        // #[column(type = <type>, "name")]
        // #[column(rename_all = "<rule>")]
        loop {
            let lookahead = input.lookahead1();

            if lookahead.peek(syn::LitStr) {
                if result.name.is_some() {
                    return Err(syn::Error::new(input.span(), "duplicate column name"));
                }
                result.name = Some(input.parse()?);
            } else if lookahead.peek(kw::rename_all) {
                if result.rename_all.is_some() {
                    return Err(syn::Error::new(input.span(), "duplicate rename_all"));
                }
                let _rename_all_token: kw::rename_all = input.parse()?;
                let _eq_token: syn::Token![=] = input.parse()?;
                let lit: syn::LitStr = input.parse()?;
                result.rename_all = Some(RenameRule::from_lit(&lit)?);
            } else if lookahead.peek(kw::variant) {
                if result.variant.is_some() {
                    return Err(syn::Error::new(
                        input.span(),
                        "duplicate variant discriminant",
                    ));
                }
                let _variant_token: kw::variant = input.parse()?;
                let _eq_token: syn::Token![=] = input.parse()?;
                // Accept either an integer literal or a string literal
                let lookahead2 = input.lookahead1();
                if lookahead2.peek(syn::LitInt) {
                    let lit: syn::LitInt = input.parse()?;
                    result.variant = Some(VariantValue::Integer(lit.base10_parse()?));
                } else if lookahead2.peek(syn::LitStr) {
                    let lit: syn::LitStr = input.parse()?;
                    let value = lit.value();
                    if value.is_empty() {
                        return Err(syn::Error::new_spanned(
                            lit,
                            "variant label must not be empty",
                        ));
                    }
                    result.variant = Some(VariantValue::String(value));
                } else {
                    return Err(lookahead2.error());
                }
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
    syn::custom_keyword!(variant);
    syn::custom_keyword!(rename_all);
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

    syn::custom_keyword!(f32);
    syn::custom_keyword!(f64);

    syn::custom_keyword!(text);
    syn::custom_keyword!(varchar);
    syn::custom_keyword!(json);
    syn::custom_keyword!(jsonb);

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
    Float(u8),
    Text,
    VarChar(u64),
    Json,
    Jsonb,
    Numeric(Option<(u32, u32)>),
    Binary(u64),
    Blob,
    Timestamp(u8),
    Date,
    Time(u8),
    DateTime(u8),
    /// Native database enum type. The optional string is a custom PostgreSQL
    /// type name; when `None`, the name is derived from the Rust enum in
    /// snake_case.
    Enum(Option<String>),
    Custom(syn::LitStr),
}

impl ColumnType {
    /// Returns `true` for `Text` and `VarChar` — the plain string storage types
    /// that opt out of native enum representation.
    pub(crate) fn is_string_like(&self) -> bool {
        matches!(self, Self::Text | Self::VarChar(_))
    }

    /// Returns the signedness and byte width of an integer storage type.
    pub(crate) fn integer_spec(&self) -> Option<(bool, u8)> {
        match self {
            Self::Integer(size) => Some((true, *size)),
            Self::UnsignedInteger(size) => Some((false, *size)),
            _ => None,
        }
    }
}

impl syn::parse::Parse for ColumnType {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(syn::LitStr) {
            let custom: syn::LitStr = input.parse()?;
            return Ok(match custom.value().to_ascii_lowercase().as_str() {
                "json" => Self::Json,
                "jsonb" => Self::Jsonb,
                _ => Self::Custom(custom),
            });
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

        peek_ident!(f32, Float(4));
        peek_ident!(f64, Float(8));

        peek_ident!(text, Text);
        peek_ident_paren_int!(varchar, VarChar);
        peek_ident!(json, Json);
        peek_ident!(jsonb, Jsonb);

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

        // `enum` or `enum("custom_type_name")`
        if lookahead.peek(syn::Token![enum]) {
            let _kw: syn::Token![enum] = input.parse()?;
            if input.peek(syn::token::Paren) {
                let content;
                parenthesized!(content in input);
                let lit: syn::LitStr = content.parse()?;
                let name = lit.value();
                if name.is_empty() {
                    return Err(syn::Error::new_spanned(
                        lit,
                        "enum type name must not be empty",
                    ));
                }
                return Ok(Self::Enum(Some(name)));
            } else {
                return Ok(Self::Enum(None));
            }
        }

        Err(lookahead.error())
    }
}

impl ColumnType {
    /// Expand to a `#toasty::storage::tag::*` token stream identifying the
    /// storage marker for this column type, or `None` when the variant has
    /// no compile-time compatibility check (e.g. `Custom`, `Numeric`,
    /// `Enum`). `toasty` is the path prefix used elsewhere in codegen, which
    /// already resolves to `codegen_support`.
    pub(crate) fn compat_marker(
        &self,
        toasty: &proc_macro2::TokenStream,
    ) -> Option<proc_macro2::TokenStream> {
        let tag = quote! { #toasty::storage::tag };
        let marker = match self {
            Self::Boolean => quote! { #tag::Boolean },
            Self::Integer(1) => quote! { #tag::I8 },
            Self::Integer(2) => quote! { #tag::I16 },
            Self::Integer(4) => quote! { #tag::I32 },
            Self::Integer(8) => quote! { #tag::I64 },
            Self::UnsignedInteger(1) => quote! { #tag::U8 },
            Self::UnsignedInteger(2) => quote! { #tag::U16 },
            Self::UnsignedInteger(4) => quote! { #tag::U32 },
            Self::UnsignedInteger(8) => quote! { #tag::U64 },
            Self::Integer(size @ 1..=8) => quote! { #tag::Int<#size> },
            Self::UnsignedInteger(size @ 1..=8) => quote! { #tag::UInt<#size> },
            Self::Float(4) => quote! { #tag::F32 },
            Self::Float(8) => quote! { #tag::F64 },
            Self::Text => quote! { #tag::Text },
            Self::VarChar(_) => quote! { #tag::VarChar },
            Self::Json => quote! { #tag::Json },
            Self::Jsonb => quote! { #tag::Jsonb },
            Self::Binary(_) => quote! { #tag::Binary },
            Self::Blob => quote! { #tag::Blob },
            Self::Timestamp(_) => quote! { #tag::Timestamp },
            Self::Date => quote! { #tag::Date },
            Self::Time(_) => quote! { #tag::Time },
            Self::DateTime(_) => quote! { #tag::DateTime },
            // No compile-time check for non-standard widths or escape hatches.
            Self::Integer(_)
            | Self::UnsignedInteger(_)
            | Self::Float(_)
            | Self::Numeric(_)
            | Self::Enum(_)
            | Self::Custom(_) => return None,
        };
        Some(marker)
    }

    /// Expand to a fully qualified `#toasty::core::schema::db::Type::...` token stream.
    pub(crate) fn expand_with(
        &self,
        toasty: &proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        match self {
            Self::Boolean => quote! { #toasty::core::schema::db::Type::Boolean },
            Self::Integer(size) => quote! { #toasty::core::schema::db::Type::Integer(#size) },
            Self::UnsignedInteger(size) => {
                quote! { #toasty::core::schema::db::Type::UnsignedInteger(#size) }
            }
            Self::Float(size) => quote! { #toasty::core::schema::db::Type::Float(#size) },
            Self::Text => quote! { #toasty::core::schema::db::Type::Text },
            Self::VarChar(size) => quote! { #toasty::core::schema::db::Type::VarChar(#size) },
            Self::Json => quote! { #toasty::core::schema::db::Type::Json },
            Self::Jsonb => quote! { #toasty::core::schema::db::Type::Jsonb },
            Self::Numeric(None) => quote! { #toasty::core::schema::db::Type::Numeric(None) },
            Self::Numeric(Some((precision, scale))) => {
                quote! { #toasty::core::schema::db::Type::Numeric(Some((#precision, #scale))) }
            }
            Self::Binary(size) => quote! { #toasty::core::schema::db::Type::Binary(#size) },
            Self::Blob => quote! { #toasty::core::schema::db::Type::Blob },
            Self::Timestamp(precision) => {
                quote! { #toasty::core::schema::db::Type::Timestamp(#precision) }
            }
            Self::Date => quote! { #toasty::core::schema::db::Type::Date },
            Self::Time(precision) => quote! { #toasty::core::schema::db::Type::Time(#precision) },
            Self::DateTime(precision) => {
                quote! { #toasty::core::schema::db::Type::DateTime(#precision) }
            }
            Self::Enum(_) => {
                // Enum storage type is constructed at the enum level with labels
                // and name, not via this generic expand path. This arm should not
                // be reached for enum types.
                panic!(
                    "ColumnType::Enum should be expanded via expand_enum_storage_ty, not expand_with"
                )
            }
            Self::Custom(custom) => {
                quote! { #toasty::core::schema::db::Type::Custom(#custom.to_string()) }
            }
        }
    }
}
