use syn::ItemFn;

/// Parsed representation of a `#[driver_test]` function
#[derive(Debug, Clone)]
pub struct DriverTest {
    /// The original function (before transformation)
    pub input: ItemFn,

    /// Test function name
    pub name: syn::Ident,

    /// List of test kinds to generate
    pub kinds: Vec<Kind>,
}

/// Kinds of test variants to generate
#[derive(Debug, Clone)]
pub enum Kind {
    /// u64 ID type variant
    IdU64,
}

impl Kind {
    /// Get the variant function name (e.g., "id_u64")
    pub fn name(&self) -> &'static str {
        match self {
            Kind::IdU64 => "id_u64",
        }
    }

    /// Get the target type for ID replacement
    pub fn target_type(&self) -> syn::Type {
        match self {
            Kind::IdU64 => syn::parse_quote!(u64),
        }
    }

    /// Get the variant name as an ident
    pub fn ident(&self) -> syn::Ident {
        syn::Ident::new(self.name(), proc_macro2::Span::call_site())
    }
}

impl DriverTest {
    /// Parse a function with the `#[driver_test]` attribute
    pub fn from_item_fn(input: ItemFn) -> Self {
        let name = input.sig.ident.clone();

        // Currently hardcoded to generate only id_u64 variant
        // TODO: Make this configurable via attribute parameters
        let kinds = vec![Kind::IdU64];

        DriverTest { input, name, kinds }
    }

    /// Get list of variant names
    pub fn variant_names(&self) -> Vec<&str> {
        self.kinds.iter().map(|k| k.name()).collect()
    }
}
