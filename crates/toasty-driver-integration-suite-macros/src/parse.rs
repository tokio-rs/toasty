use syn::{parse::Parse, punctuated::Punctuated, token::Comma, Ident, ItemFn};

/// Parsed representation of a `#[driver_test]` function
#[derive(Debug, Clone)]
pub struct DriverTest {
    /// The original function (before transformation)
    pub input: ItemFn,

    /// Test function name
    pub name: syn::Ident,

    /// List of test kinds to generate
    pub kinds: Vec<Kind>,

    /// Required capabilities for this test
    pub requires: Vec<Capability>,

    /// Non-driver_test attributes (e.g., #[should_panic], #[ignore])
    pub attrs: Vec<syn::Attribute>,
}

/// A capability requirement, optionally negated
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Capability {
    /// The capability name
    pub name: String,
    /// Whether this capability should NOT be present
    pub negated: bool,
}

/// Kinds of test variants to generate
#[derive(Debug, Clone)]
pub struct Kind {
    /// The identifier to replace (e.g., "ID")
    pub ident: String,
    /// The target type variant
    pub variant: KindVariant,
}

/// Type variants for ID replacement
#[derive(Debug, Clone)]
pub enum KindVariant {
    /// u64 ID type variant
    IdU64,
    /// UUID ID type variant
    IdUuid,
}

impl Kind {
    /// Get the variant function name (e.g., "id_u64")
    pub fn name(&self) -> &'static str {
        self.variant.name()
    }

    /// Get the target type for ID replacement
    pub fn target_type(&self) -> syn::Type {
        self.variant.target_type()
    }

    /// Get the identifier to replace
    pub fn ident(&self) -> &str {
        &self.ident
    }
}

impl KindVariant {
    /// Get the variant function name (e.g., "id_u64")
    pub fn name(&self) -> &'static str {
        match self {
            KindVariant::IdU64 => "id_u64",
            KindVariant::IdUuid => "id_uuid",
        }
    }

    /// Get the target type for ID replacement
    pub fn target_type(&self) -> syn::Type {
        match self {
            KindVariant::IdU64 => syn::parse_quote!(u64),
            KindVariant::IdUuid => syn::parse_quote!(uuid::Uuid),
        }
    }
}

/// Attribute arguments for `#[driver_test(...)]`
#[derive(Debug)]
pub struct DriverTestAttr {
    pub id_ident: Option<String>,
    pub requires: Vec<Capability>,
}

impl Parse for DriverTestAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut id_ident = None;
        let mut requires = Vec::new();

        // Parse comma-separated list of attributes
        let attrs = Punctuated::<DriverTestAttrItem, Comma>::parse_terminated(input)?;

        for attr in attrs {
            match attr {
                DriverTestAttrItem::Id(ident) => {
                    id_ident = Some(ident);
                }
                DriverTestAttrItem::Requires(caps) => {
                    requires.extend(caps);
                }
            }
        }

        Ok(DriverTestAttr { id_ident, requires })
    }
}

/// Individual attribute item
#[derive(Debug)]
enum DriverTestAttrItem {
    /// id(IDENT) - specifies test should be expanded for multiple ID types
    Id(String),
    /// requires(cap1, cap2, ...) - specifies capabilities required for this test
    Requires(Vec<Capability>),
}

impl Parse for DriverTestAttrItem {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;

        match name.to_string().as_str() {
            "id" => {
                // Parse id(IDENT)
                let content;
                syn::parenthesized!(content in input);
                let ident: Ident = content.parse()?;
                Ok(DriverTestAttrItem::Id(ident.to_string()))
            }
            "requires" => {
                // Parse requires(cap1, cap2, not(cap3), ...)
                let content;
                syn::parenthesized!(content in input);
                let caps = Punctuated::<CapabilityItem, Comma>::parse_terminated(&content)?;
                Ok(DriverTestAttrItem::Requires(
                    caps.into_iter().map(|item| item.into()).collect(),
                ))
            }
            _ => Err(syn::Error::new_spanned(
                name,
                "unknown attribute, expected `id` or `requires`",
            )),
        }
    }
}

/// A single capability item in the requires() list
enum CapabilityItem {
    /// A plain capability name
    Plain(Ident),
    /// A negated capability: not(name)
    Negated(Ident),
}

impl Parse for CapabilityItem {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();

        if lookahead.peek(Ident) {
            let ident: Ident = input.parse()?;

            // Check if this is "not"
            if ident == "not" {
                // Parse not(capability)
                let content;
                syn::parenthesized!(content in input);
                let cap_ident: Ident = content.parse()?;
                Ok(CapabilityItem::Negated(cap_ident))
            } else {
                // Plain capability
                Ok(CapabilityItem::Plain(ident))
            }
        } else {
            Err(lookahead.error())
        }
    }
}

impl From<CapabilityItem> for Capability {
    fn from(item: CapabilityItem) -> Self {
        match item {
            CapabilityItem::Plain(ident) => Capability {
                name: ident.to_string(),
                negated: false,
            },
            CapabilityItem::Negated(ident) => Capability {
                name: ident.to_string(),
                negated: true,
            },
        }
    }
}

impl DriverTest {
    /// Parse a function with the `#[driver_test]` attribute
    pub fn from_item_fn(mut input: ItemFn, attr: DriverTestAttr) -> Self {
        let name = input.sig.ident.clone();
        let requires = attr.requires;

        // Collect non-driver_test attributes to preserve them
        let attrs: Vec<_> = input
            .attrs
            .iter()
            .filter(|attr| !attr.path().is_ident("driver_test"))
            .cloned()
            .collect();

        // Remove the #[driver_test] attribute from the function, but keep all other attributes
        // (e.g., #[should_panic], #[ignore], etc.)
        input
            .attrs
            .retain(|attr| !attr.path().is_ident("driver_test"));

        // Generate variants based on attribute
        // If test has requires, always expand (even if no id specified)
        let kinds = if let Some(ident) = attr.id_ident {
            // Generate both u64 and uuid variants with the specified identifier
            vec![
                Kind {
                    ident: ident.clone(),
                    variant: KindVariant::IdU64,
                },
                Kind {
                    ident,
                    variant: KindVariant::IdUuid,
                },
            ]
        } else if !requires.is_empty() {
            // Has requires but no id() - still expand to handle capability filtering
            // Use a dummy identifier that won't match anything
            vec![
                Kind {
                    ident: String::new(),
                    variant: KindVariant::IdU64,
                },
                Kind {
                    ident: String::new(),
                    variant: KindVariant::IdUuid,
                },
            ]
        } else {
            // No id() and no requires - no expansion needed
            vec![]
        };

        DriverTest {
            input,
            name,
            kinds,
            requires,
            attrs,
        }
    }
}
