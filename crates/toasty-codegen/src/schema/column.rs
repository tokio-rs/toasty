#[derive(Debug)]
pub(crate) struct Column {
    name: Option<syn::Ident>,
    ty: Option<syn::LitStr>,
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
                result.name = input.parse()?;
            } else if lookahead.peek(syn::Token![type]) {
                if result.name.is_some() {
                    return Err(syn::Error::new(input.span(), "duplicate column type"));
                }
                let _type_token: syn::Token![type] = input.parse()?;
                let _eq_token: syn::Token![=] = input.parse()?;
                result.ty = input.parse()?;
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
